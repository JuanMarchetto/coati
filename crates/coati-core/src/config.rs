use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DesktopConfig {
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    #[serde(default = "default_true")]
    pub history_enabled: bool,
}

fn default_hotkey() -> String {
    "Ctrl+Space".into()
}
fn default_theme() -> String {
    "coati".into()
}
fn default_window_width() -> u32 {
    480
}
fn default_window_height() -> u32 {
    640
}
fn default_true() -> bool {
    true
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            hotkey: default_hotkey(),
            theme: default_theme(),
            window_width: default_window_width(),
            window_height: default_window_height(),
            history_enabled: default_true(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VoiceConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_voice_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_voice_model")]
    pub model: String,
    #[serde(default = "default_voice_language")]
    pub language: String,
}

fn default_voice_hotkey() -> String {
    "F9".into()
}
fn default_voice_model() -> String {
    "base.en".into()
}
fn default_voice_language() -> String {
    "en".into()
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            hotkey: default_voice_hotkey(),
            model: default_voice_model(),
            language: default_voice_language(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub llm: LlmConfig,
    pub tools: ToolsConfig,
    #[serde(default)]
    pub desktop: Option<DesktopConfig>,
    #[serde(default)]
    pub voice: Option<VoiceConfig>,
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
                enabled: vec![
                    "exec",
                    "read_file",
                    "list_dir",
                    "query_logs",
                    "explain_error",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
            },
            desktop: Some(DesktopConfig::default()),
            voice: Some(VoiceConfig::default()),
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
        for tool in [
            "exec",
            "read_file",
            "list_dir",
            "query_logs",
            "explain_error",
        ] {
            assert!(
                c.tools.enabled.contains(&tool.to_string()),
                "missing: {tool}"
            );
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

    #[test]
    fn parses_desktop_section() {
        let toml_str = r#"
            [llm]
            provider = "ollama"
            endpoint = "http://localhost:11434"
            model = "gemma4"
            [tools]
            enabled = ["exec"]
            [desktop]
            hotkey = "Ctrl+Alt+Space"
            theme = "coati"
            window_width = 520
            window_height = 700
            history_enabled = true
        "#;
        let c: Config = toml::from_str(toml_str).unwrap();
        let d = c.desktop.expect("desktop section");
        assert_eq!(d.hotkey, "Ctrl+Alt+Space");
        assert_eq!(d.window_width, 520);
        assert!(d.history_enabled);
    }

    #[test]
    fn default_desktop_is_sensible() {
        let d = DesktopConfig::default();
        assert_eq!(d.hotkey, "Ctrl+Space");
        assert_eq!(d.window_width, 480);
        assert_eq!(d.window_height, 640);
        assert!(d.history_enabled);
    }

    #[test]
    fn parses_voice_section() {
        let toml_str = r#"
            [llm]
            provider = "ollama"
            endpoint = "http://localhost:11434"
            model = "gemma4"
            [tools]
            enabled = ["exec"]
            [voice]
            enabled = true
            hotkey = "F9"
            model = "base.en"
            language = "en"
        "#;
        let c: Config = toml::from_str(toml_str).unwrap();
        let v = c.voice.expect("voice section");
        assert_eq!(v.hotkey, "F9");
        assert_eq!(v.model, "base.en");
        assert_eq!(v.language, "en");
        assert!(v.enabled);
    }

    #[test]
    fn default_voice_is_disabled_but_sane() {
        let v = VoiceConfig::default();
        assert!(!v.enabled, "voice must be opt-in");
        assert_eq!(v.hotkey, "F9");
        assert_eq!(v.model, "base.en");
    }
}
