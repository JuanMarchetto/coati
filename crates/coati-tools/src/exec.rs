use async_trait::async_trait;
use coati_core::{Tool, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Deserialize, JsonSchema)]
pub struct ExecInput {
    /// The program to execute (not a shell command — no piping, redirection, or variable expansion).
    pub command: String,
    /// Arguments passed to the program, one per array element.
    #[serde(default)]
    pub args: Vec<String>,
}

pub struct ExecTool {
    pub timeout_secs: u64,
}

impl Default for ExecTool {
    fn default() -> Self {
        Self { timeout_secs: 30 }
    }
}

#[async_trait]
impl Tool for ExecTool {
    type Input = ExecInput;
    const NAME: &'static str = "exec";
    const DESCRIPTION: &'static str = "Execute a program (not a shell command). Arguments are passed literally — no shell interpretation, no piping, no redirection.";

    async fn call(&self, input: ExecInput) -> Result<serde_json::Value, ToolError> {
        let fut = Command::new(&input.command).args(&input.args).output();
        let out = timeout(Duration::from_secs(self.timeout_secs), fut)
            .await
            .map_err(|_| ToolError::Execution(format!("timed out after {}s", self.timeout_secs)))?
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        Ok(json!({
            "stdout": String::from_utf8_lossy(&out.stdout),
            "stderr": String::from_utf8_lossy(&out.stderr),
            "exit_code": out.status.code().unwrap_or(-1),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn exec_runs_simple_command() {
        let tool = ExecTool::default();
        let out = tool
            .call(
                serde_json::from_value(json!({
                    "command": "echo",
                    "args": ["hello"]
                }))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(out["exit_code"], 0);
        assert!(out["stdout"].as_str().unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn exec_captures_nonzero_exit() {
        let tool = ExecTool::default();
        let out = tool
            .call(
                serde_json::from_value(json!({
                    "command": "false",
                    "args": []
                }))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(out["exit_code"], 1);
    }

    #[tokio::test]
    async fn exec_does_not_interpret_shell() {
        let tool = ExecTool::default();
        // If shell interpretation happened, this would expand $HOME. It must not.
        let out = tool
            .call(
                serde_json::from_value(json!({
                    "command": "echo",
                    "args": ["$HOME"]
                }))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(out["stdout"].as_str().unwrap().trim(), "$HOME");
    }

    #[tokio::test]
    async fn exec_times_out() {
        let tool = ExecTool { timeout_secs: 1 };
        let result = tool
            .call(
                serde_json::from_value(json!({
                    "command": "sleep",
                    "args": ["5"]
                }))
                .unwrap(),
            )
            .await;

        assert!(matches!(result, Err(coati_core::ToolError::Execution(_))));
    }
}
