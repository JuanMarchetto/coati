#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use coati_core::config::Config;
use coati_desktop::AppState;
use tauri::Manager;

mod commands;
mod shortcut;
mod stream;
mod tray;

fn main() {
    tracing_subscriber::fmt::init();
    let cfg = Config::load_or_default().unwrap_or_default();
    let state = AppState::from_config(&cfg);

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::list_models,
            commands::list_conversations,
            commands::load_conversation,
            commands::create_conversation,
            commands::send_stream,
            commands::run_proposal,
            commands::get_settings,
            commands::set_settings,
        ])
        .setup(|app| {
            tray::init(app.handle())?;
            let hotkey = {
                let state = app.state::<AppState>();
                state.hotkey.clone()
            };
            if let Err(e) = shortcut::register(app.handle(), &hotkey) {
                tracing::warn!(
                    "failed to register hotkey {hotkey}: {e}; falling back to Ctrl+Space"
                );
                let _ = shortcut::register(app.handle(), "Ctrl+Space");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
