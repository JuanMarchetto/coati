#![cfg(feature = "voice")]

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

use coati_voice::capture::PushToTalk;
use coati_voice::model;
use coati_voice::transcribe::Transcriber;

#[derive(Default)]
pub struct VoiceState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    recording: Option<PushToTalk>,
    transcriber: Option<Arc<Transcriber>>,
    model_name: String,
}

impl VoiceState {
    pub async fn on_press(&self, app: &AppHandle, model_name: &str, language: &str) -> Result<()> {
        let mut g = self.inner.lock().await;
        if g.recording.is_some() {
            return Ok(());
        }
        if g.transcriber.is_none() || g.model_name != model_name {
            let model_path: PathBuf = model::model_path(model_name);
            let model_owned = model_name.to_string();
            let lang_owned = language.to_string();
            let t = tokio::task::spawn_blocking(move || {
                Transcriber::with_language(&model_path, &lang_owned)
            })
            .await??;
            g.transcriber = Some(Arc::new(t));
            g.model_name = model_owned;
        }
        let ptt = tokio::task::spawn_blocking(PushToTalk::start).await??;
        g.recording = Some(ptt);
        let _ = app.emit("voice://recording", serde_json::json!({}));
        Ok(())
    }

    pub async fn on_release(&self, app: &AppHandle) -> Result<()> {
        let mut g = self.inner.lock().await;
        let Some(ptt) = g.recording.take() else {
            return Ok(());
        };
        let Some(transcriber) = g.transcriber.clone() else {
            let _ = app.emit("voice://idle", serde_json::json!({}));
            return Ok(());
        };
        drop(g);
        let _ = app.emit("voice://transcribing", serde_json::json!({}));

        // PushToTalk::finish() returns Vec<f32> directly (not Result) per Task 4.
        let samples = tokio::task::spawn_blocking(move || ptt.finish()).await?;
        if samples.is_empty() {
            let _ = app.emit("voice://idle", serde_json::json!({}));
            return Ok(());
        }
        let text = tokio::task::spawn_blocking(move || transcriber.transcribe(&samples)).await??;
        let _ = app.emit("voice://idle", serde_json::json!({}));
        let payload = serde_json::json!({ "text": text });
        let _ = app.emit("voice://final", payload);
        Ok(())
    }
}

pub fn voice_config(app: &AppHandle) -> (String, String) {
    use coati_desktop::AppState;
    let state = app.state::<AppState>();
    let cfg = state.config.voice.clone().unwrap_or_default();
    (cfg.model, cfg.language)
}
