#![cfg(feature = "live-model")]
// Only runs when a real whisper model is installed. CI skips this.

use coati_voice::model;
use coati_voice::transcribe::Transcriber;
use std::path::PathBuf;

#[test]
fn transcribe_silence_does_not_panic() {
    let model_path: PathBuf = model::model_path("base.en");
    if !model_path.is_file() {
        eprintln!("skipping: {} not installed", model_path.display());
        return;
    }
    let t = Transcriber::new(&model_path).unwrap();
    let samples = vec![0f32; 16_000];
    let text = t.transcribe(&samples).expect("transcribe should not error");
    assert!(text.len() < 200, "unexpectedly long output: {}", text);
}
