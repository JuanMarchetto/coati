use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug)]
pub struct LlmResponse {
    pub content: String,
    pub tool_calls: Vec<LlmToolCall>,
}

#[derive(Debug, Deserialize)]
pub struct LlmToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[(&'static str, &'static str, serde_json::Value)],
    ) -> anyhow::Result<LlmResponse>;
}

pub struct OllamaClient {
    base_url: String,
    model: String,
    http: reqwest::Client,
}

impl OllamaClient {
    pub fn new(base_url: String, model: String) -> Self {
        Self { base_url, model, http: reqwest::Client::new() }
    }
}

#[async_trait]
impl LlmProvider for OllamaClient {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[(&'static str, &'static str, serde_json::Value)],
    ) -> anyhow::Result<LlmResponse> {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
        });
        // Only include the `tools` field when there are tools to send.
        // Sending an empty array confuses some models (e.g. gemma3 returns 400).
        if !tools.is_empty() {
            let tools_json: Vec<_> = tools.iter().map(|(name, desc, schema)| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": desc,
                        "parameters": schema,
                    }
                })
            }).collect();
            body.as_object_mut().unwrap().insert("tools".into(), serde_json::Value::Array(tools_json));
        }

        let resp: serde_json::Value = self.http
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send().await?
            .error_for_status()?
            .json().await?;

        let msg = &resp["message"];
        let content = msg["content"].as_str().unwrap_or("").to_string();
        let tool_calls: Vec<LlmToolCall> = msg["tool_calls"].as_array()
            .cloned().unwrap_or_default()
            .into_iter()
            .filter_map(|tc| serde_json::from_value(tc["function"].clone()).ok())
            .collect();

        Ok(LlmResponse { content, tool_calls })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn ollama_complete_returns_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": { "role": "assistant", "content": "hi" },
                "done": true
            })))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "gemma3".into());
        let msg = ChatMessage { role: "user".into(), content: "hey".into() };
        let resp = client.complete(&[msg], &[]).await.unwrap();

        assert_eq!(resp.content, "hi");
        assert!(resp.tool_calls.is_empty());
    }

    #[tokio::test]
    async fn omits_tools_field_when_empty() {
        use std::sync::{Arc, Mutex};
        use wiremock::{Mock, MockServer, Request, ResponseTemplate};
        use wiremock::matchers::{method, path};

        // Custom matcher that captures the request body for later inspection.
        #[derive(Clone)]
        struct BodyCapture(Arc<Mutex<Option<serde_json::Value>>>);

        impl wiremock::Match for BodyCapture {
            fn matches(&self, req: &Request) -> bool {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&req.body) {
                    *self.0.lock().unwrap() = Some(v);
                }
                true // always match; we assert after the call
            }
        }

        let server = MockServer::start().await;
        let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let capture_matcher = BodyCapture(Arc::clone(&captured));

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .and(capture_matcher)
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": { "role": "assistant", "content": "ok" },
                "done": true
            })))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "gemma4".into());
        let msg = ChatMessage { role: "user".into(), content: "hello".into() };
        let resp = client.complete(&[msg], &[]).await.unwrap();
        assert_eq!(resp.content, "ok");

        let body = captured.lock().unwrap().clone().expect("body should have been captured");
        assert!(
            !body.as_object().unwrap().contains_key("tools"),
            "tools key must be absent when tools slice is empty, got: {body}"
        );
    }

    #[tokio::test]
    async fn ollama_parses_tool_calls() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {
                    "role": "assistant",
                    "content": "",
                    "tool_calls": [
                        { "function": { "name": "read_file", "arguments": { "path": "/etc/hosts" } } }
                    ]
                },
                "done": true
            })))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "gemma3".into());
        let resp = client.complete(&[], &[]).await.unwrap();

        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "read_file");
    }
}
