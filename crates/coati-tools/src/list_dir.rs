use async_trait::async_trait;
use coati_core::{Tool, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use tokio::fs;

#[derive(Deserialize, JsonSchema)]
pub struct ListDirInput {
    pub path: PathBuf,
}

pub struct ListDirTool;

#[async_trait]
impl Tool for ListDirTool {
    type Input = ListDirInput;
    const NAME: &'static str = "list_dir";
    const DESCRIPTION: &'static str =
        "List files and subdirectories in a directory (non-recursive).";

    async fn call(&self, input: ListDirInput) -> Result<serde_json::Value, ToolError> {
        let mut rd = fs::read_dir(&input.path)
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        let mut entries = Vec::new();
        while let Some(e) = rd
            .next_entry()
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?
        {
            let md = e
                .metadata()
                .await
                .map_err(|e| ToolError::Execution(e.to_string()))?;
            let kind = if md.is_dir() {
                "directory"
            } else if md.is_file() {
                "file"
            } else {
                "other"
            };
            entries.push(json!({
                "name": e.file_name().to_string_lossy(),
                "kind": kind,
                "size": md.len(),
            }));
        }

        Ok(json!({ "entries": entries }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn lists_flat_directory() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "").unwrap();
        fs::write(dir.path().join("b.txt"), "").unwrap();

        let tool = ListDirTool;
        let out = tool
            .call(
                serde_json::from_value(json!({
                    "path": dir.path().to_str().unwrap()
                }))
                .unwrap(),
            )
            .await
            .unwrap();

        let entries = out["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn distinguishes_files_and_directories() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let tool = ListDirTool;
        let out = tool
            .call(
                serde_json::from_value(json!({
                    "path": dir.path().to_str().unwrap()
                }))
                .unwrap(),
            )
            .await
            .unwrap();

        let types: Vec<&str> = out["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["kind"].as_str().unwrap())
            .collect();
        assert!(types.contains(&"file"));
        assert!(types.contains(&"directory"));
    }
}
