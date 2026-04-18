use async_trait::async_trait;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("invalid input: {0}")]
    InvalidInput(#[from] serde_json::Error),
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error("tool execution failed: {0}")]
    Execution(String),
}

#[async_trait]
pub trait Tool: Send + Sync + 'static {
    type Input: DeserializeOwned + JsonSchema + Send;
    const NAME: &'static str;
    const DESCRIPTION: &'static str;

    async fn call(&self, input: Self::Input) -> Result<serde_json::Value, ToolError>;
}

#[async_trait]
trait ErasedTool: Send + Sync {
    async fn call_json(&self, input: serde_json::Value) -> Result<serde_json::Value, ToolError>;
    fn schema(&self) -> serde_json::Value;
    fn description(&self) -> &'static str;
}

#[async_trait]
impl<T: Tool> ErasedTool for T {
    async fn call_json(&self, input: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        let typed: T::Input = serde_json::from_value(input)?;
        self.call(typed).await
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(T::Input)).unwrap()
    }
    fn description(&self) -> &'static str {
        T::DESCRIPTION
    }
}

pub struct ToolRegistry {
    tools: HashMap<&'static str, Box<dyn ErasedTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register<T: Tool>(&mut self, tool: T) {
        self.tools.insert(T::NAME, Box::new(tool));
    }

    pub async fn call(
        &self,
        name: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.into()))?;
        tool.call_json(input).await
    }

    pub fn descriptions(&self) -> Vec<(&'static str, &'static str, serde_json::Value)> {
        self.tools
            .iter()
            .map(|(name, t)| (*name, t.description(), t.schema()))
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;

    struct EchoTool;

    #[derive(serde::Deserialize, schemars::JsonSchema)]
    struct EchoInput {
        msg: String,
    }

    #[async_trait]
    impl Tool for EchoTool {
        type Input = EchoInput;
        const NAME: &'static str = "echo";
        const DESCRIPTION: &'static str = "Echoes its input back.";

        async fn call(&self, input: Self::Input) -> Result<serde_json::Value, ToolError> {
            Ok(json!({ "echoed": input.msg }))
        }
    }

    #[tokio::test]
    async fn tool_registry_dispatches_by_name() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);

        let result = registry.call("echo", json!({ "msg": "hi" })).await.unwrap();

        assert_eq!(result, json!({ "echoed": "hi" }));
    }
}
