use std::path::PathBuf;
use std::sync::Arc;

use coati_core::config::Config;
use serde::{Deserialize, Serialize};

pub mod ollama;

pub struct AppState {
    pub hotkey: String,
    pub history_enabled: bool,
    pub socket_path: PathBuf,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn from_config(cfg: &Config) -> Self {
        let desktop = cfg.desktop.clone().unwrap_or_default();
        let socket_path = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("coati/agent.sock");
        Self {
            hotkey: desktop.hotkey,
            history_enabled: desktop.history_enabled,
            socket_path,
            config: Arc::new(cfg.clone()),
        }
    }
}

#[derive(Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ConvRow {
    pub id: String,
    pub title: String,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize)]
pub struct MsgRow {
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub hotkey: String,
    pub theme: String,
    pub window_width: u32,
    pub window_height: u32,
}

#[derive(Serialize)]
pub struct RunResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub async fn list_conversations_from(
    repo: &coati_core::history::HistoryRepo,
    limit: u32,
) -> Result<Vec<ConvRow>, String> {
    repo.list_conversations(limit)
        .map_err(|e| e.to_string())
        .map(|cs| {
            cs.into_iter()
                .map(|c| ConvRow {
                    id: c.id,
                    title: c.title,
                    updated_at: c.updated_at,
                })
                .collect()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_state_from_config_picks_desktop_defaults() {
        let cfg = Config::default();
        let state = AppState::from_config(&cfg);
        assert_eq!(state.hotkey, "Ctrl+Space");
        assert!(state.history_enabled);
        assert_eq!(state.socket_path.file_name().unwrap(), "agent.sock");
    }
}

#[cfg(test)]
mod lib_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn conv_row_serializes() {
        let r = ConvRow {
            id: "a".into(),
            title: "t".into(),
            updated_at: 1,
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"id\":\"a\""));
    }

    #[tokio::test]
    async fn list_conversations_returns_rows_from_history() {
        use coati_core::history::HistoryRepo;
        let dir = TempDir::new().unwrap();
        let repo = HistoryRepo::open(&dir.path().join("h.db")).unwrap();
        repo.create_conversation("first", "gemma4").unwrap();
        let rows = list_conversations_from(&repo, 10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "first");
    }
}
