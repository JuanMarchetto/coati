use tauri::State;

use coati_desktop::{AppState, ConvRow, ModelInfo, MsgRow, RunResult, Settings};

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let endpoint = state.config.llm.endpoint.clone();
    let models = coati_desktop::ollama::list_installed(&endpoint)
        .await
        .map_err(|e| e.to_string())?;
    Ok(models
        .into_iter()
        .map(|(name, size)| ModelInfo { name, size })
        .collect())
}

#[tauri::command]
pub async fn list_conversations(_state: State<'_, AppState>) -> Result<Vec<ConvRow>, String> {
    let repo = coati_core::history::HistoryRepo::open_default().map_err(|e| e.to_string())?;
    coati_desktop::list_conversations_from(&repo, 50).await
}

#[tauri::command]
pub async fn load_conversation(
    _state: State<'_, AppState>,
    id: String,
) -> Result<Vec<MsgRow>, String> {
    let repo = coati_core::history::HistoryRepo::open_default().map_err(|e| e.to_string())?;
    let ms = repo.messages(&id).map_err(|e| e.to_string())?;
    Ok(ms
        .into_iter()
        .map(|m| MsgRow {
            role: m.role,
            content: m.content,
            created_at: m.created_at,
        })
        .collect())
}

#[tauri::command]
pub async fn create_conversation(
    state: State<'_, AppState>,
    title: String,
) -> Result<String, String> {
    let repo = coati_core::history::HistoryRepo::open_default().map_err(|e| e.to_string())?;
    let model = state.config.llm.model.clone();
    let conv = repo
        .create_conversation(&title, &model)
        .map_err(|e| e.to_string())?;
    Ok(conv.id)
}

#[tauri::command]
pub async fn send_stream(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    question: String,
    conversation_id: Option<String>,
) -> Result<(), String> {
    let sock = state.socket_path.clone();
    crate::stream::send_and_stream(&sock, app, question, conversation_id)
        .await
        .map_err(|e| e.to_string())
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
pub async fn set_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    let mut cfg = (*state.config).clone();
    cfg.desktop = Some(coati_core::config::DesktopConfig {
        hotkey: settings.hotkey,
        theme: settings.theme,
        window_width: settings.window_width,
        window_height: settings.window_height,
        history_enabled: cfg
            .desktop
            .as_ref()
            .map(|d| d.history_enabled)
            .unwrap_or(true),
    });
    cfg.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn run_proposal(
    _state: State<'_, AppState>,
    command: String,
    confirmed: bool,
) -> Result<RunResult, String> {
    if coati_desktop::proposal::needs_sudo(&command) && !confirmed {
        return Err("sudo command not confirmed".into());
    }
    let r = coati_desktop::proposal::run_confirmed(&command)
        .await
        .map_err(|e| e.to_string())?;
    Ok(RunResult {
        stdout: r.stdout,
        stderr: r.stderr,
        exit_code: r.exit_code,
    })
}
