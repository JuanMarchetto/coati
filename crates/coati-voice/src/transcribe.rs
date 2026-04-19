//! whisper-rs transcription wrapper.

use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::Mutex;

pub struct Transcriber {
    ctx: Mutex<whisper_rs::WhisperContext>,
    language: String,
}

impl Transcriber {
    pub fn new(model_path: &Path) -> Result<Self> {
        Self::with_language(model_path, "en")
    }

    pub fn with_language(model_path: &Path, language: &str) -> Result<Self> {
        if !model_path.is_file() {
            return Err(anyhow!("model not found: {}", model_path.display()));
        }
        let params = whisper_rs::WhisperContextParameters::default();
        // whisper-rs 0.13 takes &str, not &Path
        let path = model_path
            .to_str()
            .ok_or_else(|| anyhow!("non-utf8 model path"))?;
        let ctx = whisper_rs::WhisperContext::new_with_params(path, params)
            .map_err(|e| anyhow!("whisper init: {:?}", e))?;
        Ok(Self {
            ctx: Mutex::new(ctx),
            language: language.to_string(),
        })
    }

    pub fn transcribe(&self, samples_16k_mono: &[f32]) -> Result<String> {
        use whisper_rs::FullParams;
        let mut params = FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        // set_language takes Option<&'a str> tied to FullParams lifetime;
        // bind the language string locally so it outlives params.
        let lang: &str = &self.language;
        params.set_language(Some(lang));
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        params.set_translate(false);
        params.set_n_threads(num_cpus_default());

        let ctx = self.ctx.lock().map_err(|_| anyhow!("whisper ctx poisoned"))?;
        // create_state returns Result<WhisperState, WhisperError> in 0.13
        let mut state = ctx
            .create_state()
            .map_err(|e| anyhow!("whisper state: {:?}", e))?;
        // full() returns Result<c_int, WhisperError>; we discard the Ok value
        state
            .full(params, samples_16k_mono)
            .map_err(|e| anyhow!("whisper run: {:?}", e))?;
        // full_n_segments returns Result<c_int, WhisperError>
        let n = state
            .full_n_segments()
            .map_err(|e| anyhow!("whisper segments: {:?}", e))?;
        let mut out = String::new();
        for i in 0..n {
            // full_get_segment_text takes c_int
            let seg = state
                .full_get_segment_text(i)
                .map_err(|e| anyhow!("whisper segment text: {:?}", e))?;
            out.push_str(&seg);
        }
        Ok(out.trim().to_string())
    }
}

fn num_cpus_default() -> std::ffi::c_int {
    let n = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    n.min(8) as std::ffi::c_int
}
