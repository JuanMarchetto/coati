use crate::llm::{ChatMessage, LlmProvider};
use crate::tool::ToolRegistry;
use std::sync::Arc;

pub struct Agent {
    llm: Arc<dyn LlmProvider>,
    tools: ToolRegistry,
    max_iterations: usize,
}

impl Agent {
    pub fn new(llm: Arc<dyn LlmProvider>, tools: ToolRegistry) -> Self {
        Self {
            llm,
            tools,
            max_iterations: 8,
        }
    }

    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    pub async fn respond(&self, user_input: &str) -> anyhow::Result<String> {
        let mut messages = vec![ChatMessage {
            role: "user".into(),
            content: user_input.into(),
        }];

        let descriptions = self.tools.descriptions();
        let tool_descs: Vec<(&'static str, &'static str, serde_json::Value)> = descriptions
            .iter()
            .map(|(n, d, s)| (*n, *d, s.clone()))
            .collect();

        for _ in 0..self.max_iterations {
            let resp = self.llm.complete(&messages, &tool_descs).await?;

            if resp.tool_calls.is_empty() {
                return Ok(resp.content);
            }

            messages.push(ChatMessage {
                role: "assistant".into(),
                content: resp.content.clone(),
            });

            for call in resp.tool_calls {
                let result = self
                    .tools
                    .call(&call.name, call.arguments)
                    .await
                    .unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() }));
                messages.push(ChatMessage {
                    role: "tool".into(),
                    content: result.to_string(),
                });
            }
        }

        anyhow::bail!(
            "agent exceeded {} iterations without a final answer",
            self.max_iterations
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ChatMessage, LlmProvider, LlmResponse, LlmToolCall};
    use crate::tool::{Tool, ToolError, ToolRegistry};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    struct ScriptedLlm {
        responses: Mutex<Vec<LlmResponse>>,
    }

    #[async_trait]
    impl LlmProvider for ScriptedLlm {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        async fn complete(
            &self,
            _messages: &[ChatMessage],
            _tools: &[(&'static str, &'static str, serde_json::Value)],
        ) -> anyhow::Result<LlmResponse> {
            Ok(self.responses.lock().unwrap().remove(0))
        }
    }

    #[derive(serde::Deserialize, schemars::JsonSchema)]
    struct NopInput {}

    struct NopTool;

    #[async_trait]
    impl Tool for NopTool {
        type Input = NopInput;
        const NAME: &'static str = "nop";
        const DESCRIPTION: &'static str = "does nothing";
        async fn call(&self, _: NopInput) -> Result<serde_json::Value, ToolError> {
            Ok(serde_json::json!({ "result": "ok" }))
        }
    }

    #[tokio::test]
    async fn agent_returns_direct_response_when_no_tool_calls() {
        let llm = Arc::new(ScriptedLlm {
            responses: Mutex::new(vec![LlmResponse {
                content: "2 + 2 = 4".into(),
                tool_calls: vec![],
            }]),
        });
        let registry = ToolRegistry::new();
        let agent = Agent::new(llm, registry);
        let reply = agent.respond("what is 2 + 2").await.unwrap();
        assert_eq!(reply, "2 + 2 = 4");
    }

    #[tokio::test]
    async fn agent_handles_tool_call_then_final_response() {
        let llm = Arc::new(ScriptedLlm {
            responses: Mutex::new(vec![
                LlmResponse {
                    content: "".into(),
                    tool_calls: vec![LlmToolCall {
                        name: "nop".into(),
                        arguments: serde_json::json!({}),
                    }],
                },
                LlmResponse {
                    content: "all done".into(),
                    tool_calls: vec![],
                },
            ]),
        });

        let mut registry = ToolRegistry::new();
        registry.register(NopTool);

        let agent = Agent::new(llm, registry);
        let reply = agent.respond("do the thing").await.unwrap();
        assert_eq!(reply, "all done");
    }

    #[tokio::test]
    async fn agent_bails_after_max_iterations() {
        // LLM always returns tool calls, never final answer
        let llm = Arc::new(ScriptedLlm {
            responses: Mutex::new(
                (0..100)
                    .map(|_| LlmResponse {
                        content: "".into(),
                        tool_calls: vec![LlmToolCall {
                            name: "nop".into(),
                            arguments: serde_json::json!({}),
                        }],
                    })
                    .collect(),
            ),
        });
        let mut registry = ToolRegistry::new();
        registry.register(NopTool);
        let agent = Agent::new(llm, registry);
        let result = agent.respond("loop forever").await;
        assert!(result.is_err());
    }
}
