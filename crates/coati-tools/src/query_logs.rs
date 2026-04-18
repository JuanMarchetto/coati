use async_trait::async_trait;
use coati_core::{SystemLogProvider, Tool, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Deserialize, JsonSchema)]
pub struct QueryLogsInput {
    /// The service unit name (systemd unit on Linux, launchd label on macOS, etc.).
    pub unit: String,
    /// How many recent log lines to fetch. Defaults to 50, max 500.
    #[serde(default = "default_lines")]
    pub lines: u32,
}

fn default_lines() -> u32 {
    50
}

pub struct QueryLogsTool {
    provider: Arc<dyn SystemLogProvider>,
}

impl QueryLogsTool {
    pub fn new(provider: Arc<dyn SystemLogProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl Tool for QueryLogsTool {
    type Input = QueryLogsInput;
    const NAME: &'static str = "query_logs";
    const DESCRIPTION: &'static str =
        "Fetch recent log lines for a service unit. Use when diagnosing service failures.";

    async fn call(&self, input: QueryLogsInput) -> Result<serde_json::Value, ToolError> {
        let lines = self
            .provider
            .query_unit_logs(&input.unit, input.lines)
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        Ok(json!({
            "unit": input.unit,
            "lines": lines,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coati_core::SystemLogError;
    use serde_json::json;

    struct FakeProvider {
        lines: Vec<String>,
    }

    #[async_trait]
    impl SystemLogProvider for FakeProvider {
        async fn query_unit_logs(
            &self,
            unit: &str,
            _lines: u32,
        ) -> Result<Vec<String>, SystemLogError> {
            if unit == "bad" {
                return Err(SystemLogError::InvalidUnitName(unit.into()));
            }
            Ok(self.lines.clone())
        }
    }

    #[tokio::test]
    async fn returns_lines_from_provider() {
        let provider = Arc::new(FakeProvider {
            lines: vec!["line1".into(), "line2".into()],
        });
        let tool = QueryLogsTool::new(provider);
        let out = tool
            .call(
                serde_json::from_value(json!({
                    "unit": "nginx.service",
                    "lines": 10
                }))
                .unwrap(),
            )
            .await
            .unwrap();

        let lines = out["lines"].as_array().unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[tokio::test]
    async fn propagates_provider_errors() {
        let provider = Arc::new(FakeProvider { lines: vec![] });
        let tool = QueryLogsTool::new(provider);
        let result = tool
            .call(
                serde_json::from_value(json!({
                    "unit": "bad",
                    "lines": 10
                }))
                .unwrap(),
            )
            .await;

        assert!(result.is_err());
    }
}
