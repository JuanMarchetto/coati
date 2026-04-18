use async_trait::async_trait;
use coati_core::{Tool, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, JsonSchema)]
pub struct ExplainErrorInput {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub struct ExplainErrorTool;

#[async_trait]
impl Tool for ExplainErrorTool {
    type Input = ExplainErrorInput;
    const NAME: &'static str = "explain_error";
    const DESCRIPTION: &'static str = "Package a failed command's output for diagnosis. Call with the command string and its stdout/stderr/exit_code. Returns a focused analysis prompt the agent should reason over.";

    async fn call(&self, input: ExplainErrorInput) -> Result<serde_json::Value, ToolError> {
        let prompt = format!(
            "Diagnose why this command failed.\n\
             command: {}\n\
             exit_code: {}\n\
             stdout:\n{}\n\
             stderr:\n{}\n\
             Identify the root cause and propose a concrete fix.",
            input.command, input.exit_code, input.stdout, input.stderr
        );
        Ok(json!({ "analysis_prompt": prompt }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn packages_command_output_for_analysis() {
        let tool = ExplainErrorTool;
        let out = tool
            .call(
                serde_json::from_value(json!({
                    "command": "nginx -t",
                    "stdout": "",
                    "stderr": "nginx: [emerg] unknown directive \"worker_connecions\"",
                    "exit_code": 1
                }))
                .unwrap(),
            )
            .await
            .unwrap();

        let s = out["analysis_prompt"].as_str().unwrap();
        assert!(s.contains("nginx -t"));
        assert!(s.contains("worker_connecions"));
        assert!(s.contains("exit_code"));
    }
}
