use std::path::PathBuf;
use std::sync::Arc;

use coati_core::config::Config;

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
