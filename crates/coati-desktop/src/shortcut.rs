use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub fn register<R: Runtime>(app: &AppHandle<R>, accelerator: &str) -> tauri::Result<()> {
    let gs = app.global_shortcut();
    let app_clone = app.clone();
    gs.on_shortcut(accelerator, move |_app, _sc, event| {
        if event.state() == ShortcutState::Pressed {
            toggle_main(&app_clone);
        }
    })
    .map_err(|e| tauri::Error::Anyhow(anyhow::Error::new(e)))?;
    Ok(())
}

#[cfg(feature = "voice")]
pub fn register_voice<R: Runtime>(app: &AppHandle<R>, accelerator: &str) -> tauri::Result<()> {
    let gs = app.global_shortcut();
    let app_clone = app.clone();
    gs.on_shortcut(accelerator, move |_app, _sc, event| {
        let h = app_clone.clone();
        match event.state() {
            ShortcutState::Pressed => {
                tauri::async_runtime::spawn(async move {
                    let vs = h.state::<crate::voice::VoiceState>();
                    let (model, lang) = crate::voice::voice_config(&h);
                    if let Err(e) = vs.on_press(&h, &model, &lang).await {
                        tracing::warn!("voice on_press failed: {:?}", e);
                    }
                });
            }
            ShortcutState::Released => {
                tauri::async_runtime::spawn(async move {
                    let vs = h.state::<crate::voice::VoiceState>();
                    if let Err(e) = vs.on_release(&h).await {
                        tracing::warn!("voice on_release failed: {:?}", e);
                    }
                });
            }
        }
    })
    .map_err(|e| tauri::Error::Anyhow(anyhow::Error::new(e)))?;
    Ok(())
}

fn toggle_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        match w.is_visible() {
            Ok(true) => {
                let _ = w.hide();
            }
            _ => {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
    }
}
