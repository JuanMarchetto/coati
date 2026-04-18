use async_trait::async_trait;
use coati_core::{Tool, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncReadExt;

#[derive(Deserialize, JsonSchema)]
pub struct ReadFileInput {
    /// Absolute or relative path to the file.
    pub path: PathBuf,
    /// Maximum bytes to read. Defaults to 64 KiB.
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,
}

fn default_max_bytes() -> usize {
    64 * 1024
}

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    type Input = ReadFileInput;
    const NAME: &'static str = "read_file";
    const DESCRIPTION: &'static str = "Read the contents of a file up to max_bytes bytes. Use for logs, configs, source files.";

    async fn call(&self, input: ReadFileInput) -> Result<serde_json::Value, ToolError> {
        let mut file = fs::File::open(&input.path)
            .await
            .map_err(|e| ToolError::Execution(format!("open {}: {e}", input.path.display())))?;

        let mut buf = vec![0u8; input.max_bytes];
        let n = file
            .read(&mut buf)
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        let truncated = {
            let mut extra = [0u8; 1];
            file.read(&mut extra).await.map(|r| r > 0).unwrap_or(false)
        };

        buf.truncate(n);
        let content = String::from_utf8_lossy(&buf).into_owned();

        Ok(json!({
            "content": content,
            "bytes_read": n,
            "truncated": truncated,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn reads_utf8_file() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "hello world").unwrap();
        let path = f.path().to_str().unwrap().to_owned();

        let tool = ReadFileTool;
        let out = tool
            .call(
                serde_json::from_value(json!({
                    "path": path
                }))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(out["content"].as_str().unwrap().trim(), "hello world");
        assert_eq!(out["truncated"], false);
    }

    #[tokio::test]
    async fn truncates_large_files() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&vec![b'a'; 10_000]).unwrap();
        let path = f.path().to_str().unwrap().to_owned();

        let tool = ReadFileTool;
        let out = tool
            .call(
                serde_json::from_value(json!({
                    "path": path,
                    "max_bytes": 100
                }))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(out["content"].as_str().unwrap().len(), 100);
        assert_eq!(out["truncated"], true);
    }

    #[tokio::test]
    async fn rejects_missing_file() {
        let tool = ReadFileTool;
        let result = tool
            .call(
                serde_json::from_value(json!({
                    "path": "/nonexistent/file/path"
                }))
                .unwrap(),
            )
            .await;

        assert!(result.is_err());
    }
}
