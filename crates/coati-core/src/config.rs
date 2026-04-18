use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub llm: LlmConfig,
    pub tools: ToolsConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ToolsConfig {
    #[serde(default)]
    pub enabled: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            llm: LlmConfig {
                provider: "ollama".into(),
                endpoint: "http://localhost:11434".into(),
                model: "gemma4".into(),
            },
            tools: ToolsConfig {
                enabled: vec!["exec", "read_file", "list_dir", "query_logs", "explain_error"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
            },
        }
    }
}

impl Config {
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("coati/config.toml")
    }

    pub fn load_or_default() -> anyhow::Result<Self> {
        let path = Self::default_path();
        if path.exists() {
            let s = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&s)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::default_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_config() {
        let toml_str = r#"
            [llm]
            provider = "ollama"
            endpoint = "http://localhost:11434"
            model = "gemma3"

            [tools]
            enabled = ["exec", "read_file"]
        "#;
        let c: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(c.llm.model, "gemma3");
        assert!(c.tools.enabled.contains(&"exec".to_string()));
        assert!(c.tools.enabled.contains(&"read_file".to_string()));
    }

    #[test]
    fn default_has_all_tools_enabled() {
        let c = Config::default();
        for tool in ["exec", "read_file", "list_dir", "query_logs", "explain_error"] {
            assert!(c.tools.enabled.contains(&tool.to_string()), "missing: {tool}");
        }
    }

    #[test]
    fn round_trip_serialization() {
        let c = Config::default();
        let s = toml::to_string_pretty(&c).unwrap();
        let parsed: Config = toml::from_str(&s).unwrap();
        assert_eq!(parsed.llm.model, c.llm.model);
        assert_eq!(parsed.tools.enabled, c.tools.enabled);
    }
}
