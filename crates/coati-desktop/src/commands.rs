use serde::{Deserialize, Serialize};
use tauri::State;

use coati_desktop::AppState;

#[derive(Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: u64,
}

#[tauri::command]
pub async fn list_models(_state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    Ok(vec![])
}

#[derive(Serialize, Deserialize)]
pub struct ConvRow {
    pub id: String,
    pub title: String,
    pub updated_at: i64,
}

#[tauri::command]
pub async fn list_conversations(_state: State<'_, AppState>) -> Result<Vec<ConvRow>, String> {
    Ok(vec![])
}

#[derive(Serialize, Deserialize)]
pub struct MsgRow {
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

#[tauri::command]
pub async fn load_conversation(
    _state: State<'_, AppState>,
    _id: String,
) -> Result<Vec<MsgRow>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn create_conversation(
    _state: State<'_, AppState>,
    _title: String,
) -> Result<String, String> {
    Ok(String::new())
}

#[tauri::command]
pub async fn send_stream(
    _state: State<'_, AppState>,
    _question: String,
    _conversation_id: Option<String>,
) -> Result<(), String> {
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub hotkey: String,
    pub theme: String,
    pub window_width: u32,
    pub window_height: u32,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let d = state.config.desktop.clone().unwrap_or_default();
    Ok(Settings {
        hotkey: d.hotkey,
        theme: d.theme,
        window_width: d.window_width,
        window_height: d.window_height,
    })
}

#[tauri::command]
pub async fn set_settings(_state: State<'_, AppState>, _settings: Settings) -> Result<(), String> {
    Ok(())
}

#[derive(Serialize)]
pub struct RunResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[tauri::command]
pub async fn run_proposal(
    _state: State<'_, AppState>,
    _command: String,
    _confirmed: bool,
) -> Result<RunResult, String> {
    Ok(RunResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
    })
}
