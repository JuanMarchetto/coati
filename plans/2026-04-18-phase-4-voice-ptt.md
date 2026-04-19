# Phase 4 — Voice (Push-to-Talk MVP) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hold F9 to record, release to transcribe locally with whisper.cpp, pipe the transcript into the existing Desktop chat as an `AskStream` request. Audio never leaves the machine. CLI users are never forced to pull whisper-rs or cpal.

**Architecture:** New `coati-voice` crate (audio capture + whisper-rs wrapper + model manager). Optional feature `voice` on both `coati-cli` and `coati-desktop` — default off. First-run downloads `ggml-base.en.bin` (SHA-256 verified) to `~/.local/share/coati/models/`. Desktop registers F9 as a second global shortcut, emits `voice://recording` + `voice://idle` events to the webview for the recording indicator, and on release transcribes and dispatches the text through the existing stream pipeline.

**Tech Stack:**
- `whisper-rs = "0.13"` (safe bindings around whisper.cpp)
- `cpal = "0.15"` (portable audio capture)
- `hound = "3.5"` (WAV read/write for CLI tool + fixtures)
- `sha2 = "0.10"` (SHA-256 verify on model download)
- Existing: tauri-plugin-global-shortcut (already registered in Phase 3)

---

## Global Constraints (read before any task)

1. **Local-first is hard.** Audio samples never serialize out of the process. The only network call Phase 4 adds is the *explicit user-accepted* model download in Task 3.
2. **Feature gates everywhere.** Nothing in `coati-core` depends on `coati-voice`. `cargo check --workspace` with no features must still succeed on a machine without `libasound2-dev` or `clang`.
3. **Rust 1.82 toolchain pin holds.** If a transitive dep needs Rust 1.85, tighten with `=` version spec (same playbook as Phase 3's Tauri pins).
4. **No new hard deps in coati-desktop's default set.** If desktop gains PTT code, it goes behind `--features voice`, which chain-enables the coati-voice dep.
5. **Hotkey:** F9 default, configurable via `[voice] hotkey` in `~/.config/coati/config.toml`. Must not collide with the Phase 3 chat hotkey (Ctrl+Space).
6. **Phase 2 autopilot constraint stays:** voice can *request* the chat window's existing propose flow (which still requires the No/Yes confirm for sudo). Voice never bypasses that confirm.

---

## File Structure

**New:**
- `crates/coati-voice/` — new workspace member
  - `Cargo.toml`
  - `src/lib.rs` — module roots + `anyhow::Error` reexport
  - `src/model.rs` — model manifest + `download_model(name, destination, progress_cb)` with SHA-256 verify
  - `src/capture.rs` — `PushToTalk::start` → returns a handle, `finish() -> Vec<f32>` yielding mono 16kHz samples
  - `src/transcribe.rs` — `Transcriber::new(model_path)` + `transcribe(&[f32]) -> anyhow::Result<String>`
  - `tests/fixtures/hello.wav` — 1s 16kHz mono fixture (record or ship a generated sine+noise file)
  - `tests/integration_transcribe.rs` — loads fixture, asserts non-empty transcript
- `crates/coati-cli/src/cmd_voice.rs` — `coati voice {setup,transcribe}` subcommands
- `crates/coati-desktop/src/voice.rs` — F9 hold handler, event emitters
- `crates/coati-desktop/dist/voice.js` — tiny module consumed by `app.js` for the recording banner
- `tests/e2e/voice.sh` — dogfood smoke script (silence → near-empty transcript)

**Modified:**
- `Cargo.toml` (workspace) — add `coati-voice`, pin whisper-rs/cpal/hound/sha2
- `crates/coati-core/src/config.rs` — add `VoiceConfig`
- `crates/coati-cli/Cargo.toml` — optional `coati-voice` behind `voice` feature
- `crates/coati-cli/src/main.rs` — register `Voice` subcommand under `--features voice`
- `crates/coati-desktop/Cargo.toml` — optional `coati-voice` behind `voice` feature (which chain-enables the existing `desktop` feature)
- `crates/coati-desktop/src/main.rs` — call `voice::register` when compiled with `voice`
- `crates/coati-desktop/src/shortcut.rs` — parse & register F9 alongside existing chat hotkey; keep a shared `toggle_main` call
- `crates/coati-desktop/dist/app.js` — listen for `voice://recording`, render banner + block text input while recording; on `voice://final` inject text into the input and auto-submit
- `crates/coati-desktop/dist/index.html` — add `<div id="rec-banner" hidden>` markup
- `.github/workflows/ci.yml` — install `libasound2-dev`, `clang`, `pkg-config` on desktop job; add a `voice` job that builds + tests `coati-voice`
- `scripts/install-desktop.sh` — accept `--with-voice` flag, pass through to `--features voice`
- `README.md` — Voice section with setup + demo
- `ROADMAP.md` — mark Phase 4 shipped
- `CLAUDE.md` — append Phase 4 status line

---

## Task 1: Scaffold `coati-voice` crate + [voice] config section

**Files:**
- Create: `crates/coati-voice/Cargo.toml`
- Create: `crates/coati-voice/src/lib.rs`
- Modify: `Cargo.toml` (workspace)
- Modify: `crates/coati-core/src/config.rs`

- [ ] **Step 1: Write the failing config test**

Append to `crates/coati-core/src/config.rs` tests module:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p coati-core config::tests::parses_voice_section`
Expected: compile error — `VoiceConfig` not in scope, `Config` has no `voice` field.

- [ ] **Step 3: Add VoiceConfig to config.rs**

Insert after the `DesktopConfig` block, before the `Config` struct:

```rust
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
```

Then extend `Config`:

```rust
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub llm: LlmConfig,
    pub tools: ToolsConfig,
    #[serde(default)]
    pub desktop: Option<DesktopConfig>,
    #[serde(default)]
    pub voice: Option<VoiceConfig>,
}
```

And extend `Config::default()` to set `voice: Some(VoiceConfig::default())`.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p coati-core`
Expected: all config tests pass (incl. new two).

- [ ] **Step 5: Create coati-voice workspace member**

Write `crates/coati-voice/Cargo.toml`:

```toml
[package]
name = "coati-voice"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
rust-version.workspace = true

[dependencies]
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true
serde = { workspace = true }
serde_json = { workspace = true }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "stream"] }
tokio = { workspace = true }
futures-util = "0.3"
sha2 = "0.10"
hound = "3.5"
cpal = "0.15"
whisper-rs = "0.13"
dirs = "5"

[dev-dependencies]
tempfile = "3"
wiremock = "0.5.22"
```

Write `crates/coati-voice/src/lib.rs`:

```rust
//! Local voice capture + whisper.cpp transcription.

pub mod capture;
pub mod model;
pub mod transcribe;

pub use anyhow::{Error, Result};
```

Create empty stubs so the crate compiles:

`crates/coati-voice/src/model.rs`:

```rust
//! Model manifest and downloader. Real implementation lands in Task 2.
```

`crates/coati-voice/src/capture.rs`:

```rust
//! cpal audio capture. Real implementation lands in Task 4.
```

`crates/coati-voice/src/transcribe.rs`:

```rust
//! whisper-rs wrapper. Real implementation lands in Task 5.
```

- [ ] **Step 6: Add to workspace**

Edit `Cargo.toml` (workspace root), extend the members array:

```toml
members = [
    "crates/coati-core",
    "crates/coati-tools",
    "crates/coati-cli",
    "crates/coati-hw",
    "crates/coati-desktop",
    "crates/coati-voice",
]
```

- [ ] **Step 7: Verify it builds**

Run: `cargo check -p coati-voice`
Expected: builds cleanly (might emit unused-import warnings — fine for now).

- [ ] **Step 8: Commit**

```bash
git add crates/coati-voice Cargo.toml crates/coati-core/src/config.rs
git commit -m "feat(voice): scaffold coati-voice crate and [voice] config section"
```

---

## Task 2: Model manifest + SHA-256 downloader

**Files:**
- Modify: `crates/coati-voice/src/model.rs`
- Test: `crates/coati-voice/src/model.rs` (inline `#[cfg(test)] mod tests`)

**Context:** We support two models. URLs point to the canonical Hugging Face GGML mirror maintained by ggerganov. SHA-256s are from the `ggml-*.bin` sha256 list in that repo as of 2026-04. If a subagent finds drift, update in-place — these MUST match at implementation time.

| name | size | URL | SHA-256 |
|------|------|-----|---------|
| `base.en` | ~148 MB | `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin` | `a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002` |
| `tiny.en` | ~75 MB | `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin` | `921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f` |

If the subagent cannot verify the SHA-256s against upstream at implementation time, STOP and ask the controller — shipping the wrong hash means every user's first-run download silently fails.

- [ ] **Step 1: Write the failing test**

Replace `src/model.rs` with the following (keep the doc comment at top):

```rust
//! Model manifest and downloader.

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    pub name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size_mb: u32,
}

pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        name: "base.en",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
        sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
        size_mb: 148,
    },
    ModelSpec {
        name: "tiny.en",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
        sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
        size_mb: 75,
    },
];

pub fn lookup(name: &str) -> Option<&'static ModelSpec> {
    MODELS.iter().find(|m| m.name == name)
}

pub fn default_models_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share"))
        .join("coati/models")
}

pub fn model_path(name: &str) -> PathBuf {
    default_models_dir().join(format!("ggml-{}.bin", name))
}

pub fn is_installed(name: &str) -> bool {
    model_path(name).is_file()
}

/// Stream-download a model to `dest`, verifying SHA-256 during the stream.
/// `on_progress(bytes_so_far, total_bytes_opt)` is invoked periodically.
pub async fn download<F>(
    spec: &ModelSpec,
    dest: &Path,
    base_url_override: Option<&str>,
    mut on_progress: F,
) -> Result<()>
where
    F: FnMut(u64, Option<u64>),
{
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let url = match base_url_override {
        Some(base) => format!("{}/ggml-{}.bin", base.trim_end_matches('/'), spec.name),
        None => spec.url.to_string(),
    };

    let client = reqwest::Client::builder().build()?;
    let resp = client.get(&url).send().await?.error_for_status()?;
    let total = resp.content_length();
    let mut stream = resp.bytes_stream();
    let tmp = dest.with_extension("partial");
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut hasher = Sha256::new();
    let mut seen: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        hasher.update(&bytes);
        file.write_all(&bytes).await?;
        seen += bytes.len() as u64;
        on_progress(seen, total);
    }
    file.flush().await?;
    drop(file);

    let got = hex_lower(&hasher.finalize());
    if got != spec.sha256 {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(anyhow!(
            "SHA-256 mismatch for {}: expected {}, got {}",
            spec.name,
            spec.sha256,
            got
        ));
    }

    tokio::fs::rename(&tmp, dest).await?;
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_model() {
        let m = lookup("base.en").expect("base.en should exist");
        assert_eq!(m.name, "base.en");
        assert!(m.sha256.len() == 64);
    }

    #[test]
    fn lookup_unknown_is_none() {
        assert!(lookup("huge.xyz").is_none());
    }

    #[test]
    fn model_path_is_under_data_dir() {
        let p = model_path("base.en");
        assert!(p.ends_with("ggml-base.en.bin"));
        assert!(p.to_string_lossy().contains("coati/models"));
    }

    #[tokio::test]
    async fn download_rejects_sha_mismatch() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"not-a-model" as &[u8]))
            .mount(&server)
            .await;

        let spec = ModelSpec {
            name: "test.en",
            url: "unused",
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
            size_mb: 0,
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let dest = tmp.path().join("ggml-test.en.bin");
        let err = download(&spec, &dest, Some(&server.uri()), |_, _| {})
            .await
            .unwrap_err();
        assert!(err.to_string().contains("SHA-256 mismatch"));
        assert!(!dest.exists(), "partial file must not be promoted on mismatch");
    }

    #[tokio::test]
    async fn download_success_writes_file_and_reports_progress() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let body: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
        let mut hasher = Sha256::new();
        hasher.update(&body);
        let sha = hex_lower(&hasher.finalize());
        let sha_static: &'static str = Box::leak(sha.into_boxed_str());

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&server)
            .await;

        let spec = ModelSpec {
            name: "test.en",
            url: "unused",
            sha256: sha_static,
            size_mb: 0,
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let dest = tmp.path().join("ggml-test.en.bin");
        let mut saw_progress = false;
        download(&spec, &dest, Some(&server.uri()), |n, _| {
            if n > 0 {
                saw_progress = true;
            }
        })
        .await
        .unwrap();
        assert!(dest.is_file());
        assert_eq!(tokio::fs::read(&dest).await.unwrap(), body);
        assert!(saw_progress);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p coati-voice`
Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/coati-voice/src/model.rs
git commit -m "feat(voice): model manifest + SHA-256 verified downloader"
```

---

## Task 3: `coati voice setup` CLI subcommand

**Files:**
- Create: `crates/coati-cli/src/cmd_voice.rs`
- Modify: `crates/coati-cli/Cargo.toml` — add optional `coati-voice` dep + `voice` feature
- Modify: `crates/coati-cli/src/main.rs` — register subcommand behind `cfg(feature = "voice")`
- Test: `crates/coati-cli/tests/voice_setup.rs`

- [ ] **Step 1: Write the failing integration test**

`crates/coati-cli/tests/voice_setup.rs`:

```rust
#![cfg(feature = "voice")]

use assert_cmd::Command;

#[test]
fn voice_setup_no_accept_prints_help() {
    let mut cmd = Command::cargo_bin("coati").unwrap();
    cmd.args(["voice", "setup", "--model", "tiny.en"]);
    // No --yes, so it should print a prompt banner and exit 1 for a non-TTY stdin.
    let output = cmd.assert();
    output.failure().stdout(predicates::str::contains("Would download"));
}
```

And list `predicates` + `assert_cmd` dev-deps if not already present. `assert_cmd` is already listed in `crates/coati-cli/Cargo.toml` (Phase 1).

- [ ] **Step 2: Run it**

Run: `cargo test -p coati-cli --features voice voice_setup_no_accept_prints_help`
Expected: fails — feature does not exist.

- [ ] **Step 3: Wire the feature + dep**

Edit `crates/coati-cli/Cargo.toml`. Add:

```toml
[features]
default = []
voice = ["dep:coati-voice"]

[dependencies]
# ... existing deps ...
coati-voice = { path = "../coati-voice", optional = true }
```

And add to `[dev-dependencies]`: `predicates = "3"`.

- [ ] **Step 4: Create cmd_voice.rs**

```rust
use anyhow::{anyhow, Result};
use coati_voice::model::{self, MODELS};
use std::io::{self, BufRead, Write};

pub async fn setup(model_name: &str, yes: bool) -> Result<()> {
    let spec = model::lookup(model_name)
        .ok_or_else(|| anyhow!("unknown model '{}' (try: {})", model_name, known_models()))?;

    let dest = model::model_path(spec.name);
    if dest.is_file() {
        println!("Model {} already installed at {}", spec.name, dest.display());
        return Ok(());
    }

    println!("Would download {} (~{} MB) from {}", spec.name, spec.size_mb, spec.url);
    println!("  -> {}", dest.display());
    println!("Audio and transcripts stay local. This download is the only network call.");

    if !yes {
        print!("Proceed? [y/N]: ");
        io::stdout().flush()?;
        let mut line = String::new();
        let stdin = io::stdin();
        let n = stdin.lock().read_line(&mut line).unwrap_or(0);
        let answer = line.trim().to_lowercase();
        if n == 0 || (answer != "y" && answer != "yes") {
            return Err(anyhow!("aborted"));
        }
    }

    let pb_total = std::cell::Cell::new(0u64);
    model::download(spec, &dest, None, |seen, total| {
        if let Some(t) = total {
            if pb_total.get() == 0 {
                pb_total.set(t);
            }
            let pct = (seen as f64 / t as f64) * 100.0;
            print!("\rDownloading: {:>5.1}% ({}/{} bytes)", pct, seen, t);
        } else {
            print!("\rDownloading: {} bytes", seen);
        }
        let _ = io::stdout().flush();
    })
    .await?;
    println!("\nInstalled {} at {}", spec.name, dest.display());
    Ok(())
}

pub async fn transcribe_file(path: &std::path::Path, model_name: &str) -> Result<()> {
    use coati_voice::transcribe::Transcriber;
    let spec = model::lookup(model_name)
        .ok_or_else(|| anyhow!("unknown model '{}'", model_name))?;
    let model_path = model::model_path(spec.name);
    if !model_path.is_file() {
        return Err(anyhow!(
            "model {} is not installed — run `coati voice setup --model {}`",
            spec.name,
            spec.name
        ));
    }
    let samples = load_wav_mono16k(path)?;
    let t = Transcriber::new(&model_path)?;
    let text = t.transcribe(&samples)?;
    println!("{}", text);
    Ok(())
}

fn load_wav_mono16k(path: &std::path::Path) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.sample_rate != 16_000 {
        return Err(anyhow!(
            "expected 16kHz WAV, got {} Hz — re-record or ffmpeg-convert",
            spec.sample_rate
        ));
    }
    if spec.channels != 1 {
        return Err(anyhow!("expected mono WAV, got {} channels", spec.channels));
    }
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| anyhow!("wav read: {}", e))?,
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample as f32;
            let scale = 2f32.powf(bits - 1.0);
            reader
                .samples::<i32>()
                .map(|r| r.map(|v| v as f32 / scale))
                .collect::<Result<_, _>>()
                .map_err(|e| anyhow!("wav read: {}", e))?
        }
    };
    Ok(samples)
}

fn known_models() -> String {
    MODELS
        .iter()
        .map(|m| m.name)
        .collect::<Vec<_>>()
        .join(", ")
}
```

Add to `crates/coati-cli/Cargo.toml` `[dependencies]`:

```toml
hound = "3.5"
```

(Even though it's also pulled in transitively by coati-voice; the CLI file reader uses it directly.)

- [ ] **Step 5: Register in main.rs**

Edit `main.rs`. Add at the top of the file:

```rust
#[cfg(feature = "voice")]
mod cmd_voice;
```

Extend `enum Commands` (inside the existing block) with a cfg-gated arm:

```rust
    /// Voice commands (requires --features voice at build time).
    #[cfg(feature = "voice")]
    #[command(subcommand)]
    Voice(VoiceAction),
```

Add a new enum:

```rust
#[cfg(feature = "voice")]
#[derive(Subcommand)]
enum VoiceAction {
    /// Download and install a whisper model.
    Setup {
        #[arg(long, default_value = "base.en")]
        model: String,
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Transcribe a 16kHz mono WAV file and print the text.
    Transcribe {
        path: std::path::PathBuf,
        #[arg(long, default_value = "base.en")]
        model: String,
    },
}
```

Extend the match in `main`:

```rust
        #[cfg(feature = "voice")]
        Commands::Voice(action) => match action {
            VoiceAction::Setup { model, yes } => cmd_voice::setup(&model, yes).await,
            VoiceAction::Transcribe { path, model } => cmd_voice::transcribe_file(&path, &model).await,
        },
```

- [ ] **Step 6: Run the test**

Run: `cargo test -p coati-cli --features voice voice_setup_no_accept_prints_help`
Expected: pass.

- [ ] **Step 7: Verify default build still works**

Run: `cargo check -p coati-cli` (no features)
Expected: compiles cleanly, `cmd_voice` and `Voice` subcommand absent.

- [ ] **Step 8: Commit**

```bash
git add crates/coati-cli/src/cmd_voice.rs crates/coati-cli/src/main.rs \
        crates/coati-cli/Cargo.toml crates/coati-cli/tests/voice_setup.rs
git commit -m "feat(voice): coati voice setup + transcribe CLI subcommands"
```

---

## Task 4: cpal audio capture (16kHz mono f32)

**Files:**
- Modify: `crates/coati-voice/src/capture.rs`

**Design:** `PushToTalk::start()` opens the default input stream, spawns a pump thread that pushes f32 samples into a `std::sync::mpsc::Sender`, and returns a handle. `finish()` drops the stream, drains the channel, and returns a resampled Vec<f32> at 16kHz mono. Resampling is simple linear (good enough for whisper; piper-quality not needed).

- [ ] **Step 1: Write the failing test**

Append to `crates/coati-voice/src/capture.rs`:

```rust
//! cpal audio capture (push-to-talk).

use anyhow::{anyhow, Result};
use std::sync::mpsc;
use std::thread;

pub struct PushToTalk {
    rx: mpsc::Receiver<Vec<f32>>,
    stream_thread: Option<thread::JoinHandle<()>>,
    stop_tx: Option<mpsc::Sender<()>>,
    input_sample_rate: u32,
    input_channels: u16,
}

impl PushToTalk {
    pub fn start() -> Result<Self> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("no default input device"))?;
        let cfg = device.default_input_config()?;
        let input_sample_rate = cfg.sample_rate().0;
        let input_channels = cfg.channels();

        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let sample_format = cfg.sample_format();
        let stream_cfg: cpal::StreamConfig = cfg.into();

        let stream_thread = thread::spawn(move || {
            let err_fn = |e| tracing::error!("cpal stream error: {}", e);
            let tx_f32 = tx.clone();
            let stream_result = match sample_format {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    &stream_cfg,
                    move |data: &[f32], _: &_| {
                        let _ = tx_f32.send(data.to_vec());
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => device.build_input_stream(
                    &stream_cfg,
                    move |data: &[i16], _: &_| {
                        let buf: Vec<f32> =
                            data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                        let _ = tx_f32.send(buf);
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::U16 => device.build_input_stream(
                    &stream_cfg,
                    move |data: &[u16], _: &_| {
                        let buf: Vec<f32> = data
                            .iter()
                            .map(|s| (*s as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0))
                            .collect();
                        let _ = tx_f32.send(buf);
                    },
                    err_fn,
                    None,
                ),
                other => {
                    tracing::error!("unsupported sample format: {:?}", other);
                    return;
                }
            };
            let stream = match stream_result {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("build_input_stream: {}", e);
                    return;
                }
            };
            if let Err(e) = stream.play() {
                tracing::error!("stream.play(): {}", e);
                return;
            }
            let _ = stop_rx.recv();
            drop(stream);
        });

        Ok(Self {
            rx,
            stream_thread: Some(stream_thread),
            stop_tx: Some(stop_tx),
            input_sample_rate,
            input_channels,
        })
    }

    /// Stop capture, return 16kHz mono f32 samples suitable for whisper-rs.
    pub fn finish(mut self) -> Result<Vec<f32>> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(t) = self.stream_thread.take() {
            let _ = t.join();
        }
        let mut raw = Vec::new();
        while let Ok(chunk) = self.rx.try_recv() {
            raw.extend(chunk);
        }
        Ok(to_mono_16k(
            &raw,
            self.input_sample_rate,
            self.input_channels,
        ))
    }
}

/// Downmix to mono and linearly resample to 16kHz.
pub fn to_mono_16k(samples: &[f32], input_rate: u32, channels: u16) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    let mono: Vec<f32> = if channels <= 1 {
        samples.to_vec()
    } else {
        samples
            .chunks(channels as usize)
            .map(|c| c.iter().sum::<f32>() / channels as f32)
            .collect()
    };
    if input_rate == 16_000 {
        return mono;
    }
    let target = 16_000f64;
    let ratio = target / input_rate as f64;
    let out_len = (mono.len() as f64 * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let lo = src.floor() as usize;
        let hi = (lo + 1).min(mono.len() - 1);
        let frac = src - lo as f64;
        let s = mono[lo] as f64 * (1.0 - frac) + mono[hi] as f64 * frac;
        out.push(s as f32);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_48k_stereo_to_16k_mono_roughly_third_length() {
        let stereo: Vec<f32> = (0..4800).flat_map(|i| [i as f32, i as f32]).collect();
        let out = to_mono_16k(&stereo, 48_000, 2);
        // 4800 frames at 48k -> 1600 frames at 16k.
        assert!((out.len() as i64 - 1600).abs() <= 1, "got {}", out.len());
    }

    #[test]
    fn resample_16k_mono_is_passthrough() {
        let input: Vec<f32> = (0..1600).map(|i| i as f32).collect();
        let out = to_mono_16k(&input, 16_000, 1);
        assert_eq!(out.len(), input.len());
        assert_eq!(out[500], input[500]);
    }

    #[test]
    fn empty_is_empty() {
        assert!(to_mono_16k(&[], 48_000, 2).is_empty());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p coati-voice capture::`
Expected: 3 tests pass. (The live-mic `PushToTalk` code compiles but isn't exercised in unit tests — smoke test in Task 11 covers it.)

- [ ] **Step 3: Commit**

```bash
git add crates/coati-voice/src/capture.rs
git commit -m "feat(voice): cpal PTT capture + mono/16k resampler"
```

---

## Task 5: whisper-rs transcription wrapper

**Files:**
- Modify: `crates/coati-voice/src/transcribe.rs`
- Create: `crates/coati-voice/tests/fixtures/hello.wav` (generated at test time by a build helper — see Step 2)

**Context:** We generate a synthetic WAV in the test itself (silence + tone) rather than checking in a binary file — keeps the repo small and avoids licensing questions. We don't need whisper to output "hello" — we just need it to run end-to-end without erroring, and produce *some* string (possibly empty) in under a reasonable time.

- [ ] **Step 1: Write the failing integration test**

`crates/coati-voice/tests/integration_transcribe.rs`:

```rust
#![cfg(feature = "live-model")]
// This test only runs when a real whisper model is installed locally. CI
// skips it by not enabling the feature. Dogfood script in Task 11 covers
// the end-to-end mic path.

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
    let samples = vec![0f32; 16_000]; // 1 second of silence at 16kHz.
    let text = t.transcribe(&samples).expect("transcribe should not error");
    // whisper may output "" or "[BLANK_AUDIO]" or similar for silence — all fine.
    assert!(text.len() < 200, "unexpectedly long output: {}", text);
}
```

Add to `crates/coati-voice/Cargo.toml`:

```toml
[features]
live-model = []
```

- [ ] **Step 2: Write the wrapper**

Replace `src/transcribe.rs`:

```rust
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
        params.set_language(Some(&self.language));
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        params.set_translate(false);
        params.set_n_threads(num_cpus_default());

        let ctx = self.ctx.lock().map_err(|_| anyhow!("whisper ctx poisoned"))?;
        let mut state = ctx
            .create_state()
            .map_err(|e| anyhow!("whisper state: {:?}", e))?;
        state
            .full(params, samples_16k_mono)
            .map_err(|e| anyhow!("whisper run: {:?}", e))?;
        let n = state
            .full_n_segments()
            .map_err(|e| anyhow!("whisper segments: {:?}", e))?;
        let mut out = String::new();
        for i in 0..n {
            let seg = state
                .full_get_segment_text(i)
                .map_err(|e| anyhow!("whisper segment text: {:?}", e))?;
            out.push_str(&seg);
        }
        Ok(out.trim().to_string())
    }
}

fn num_cpus_default() -> std::os::raw::c_int {
    let n = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    n.min(8) as std::os::raw::c_int
}
```

Note: `whisper-rs 0.13` exposes exactly this API surface. If the subagent hits a version mismatch (rename of `WhisperContext::new_with_params` etc.), check `cargo doc --open -p whisper-rs` against the installed version and adapt.

- [ ] **Step 3: Run tests**

Run: `cargo check -p coati-voice --all-features`
Expected: compiles. (The `live-model` test is gated, so `cargo test` without that feature just runs the capture + model tests.)

Run: `cargo test -p coati-voice`
Expected: 5 + 3 = 8 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/coati-voice/src/transcribe.rs crates/coati-voice/Cargo.toml \
        crates/coati-voice/tests/integration_transcribe.rs
git commit -m "feat(voice): whisper-rs Transcriber wrapper"
```

---

## Task 6: Smoke the CLI manually (no subagent step — controller task)

> Executed by the controller before dispatching Task 7. Skip if whisper-rs builds are breaking on CI — Task 9 needs to land first.

**What:** Build, `coati voice setup --model tiny.en --yes`, record a short WAV with `arecord`, `coati voice transcribe /tmp/hello.wav --model tiny.en`. Controller confirms output is sane. No commit.

---

## Task 7: Desktop F9 hold-to-talk integration

**Files:**
- Create: `crates/coati-desktop/src/voice.rs`
- Modify: `crates/coati-desktop/Cargo.toml` — add `voice` feature, optional `coati-voice`
- Modify: `crates/coati-desktop/src/main.rs` — register voice under the feature gate
- Modify: `crates/coati-desktop/src/shortcut.rs` — parse a second hotkey (F9), dispatch by code in the handler
- Test: `crates/coati-desktop/src/voice.rs` inline tests for the tiny state machine only

**Design notes:**
- The Tauri `on_shortcut` callback receives a `&Shortcut` with the key that fired. We match by stringifying the shortcut against the two known keys.
- Hold-to-talk requires `ShortcutState::Pressed` *and* `ShortcutState::Released`. tauri-plugin-global-shortcut 2.0 emits both.
- Recording runs on a tokio blocking-task thread so the PTT stream thread has a stable home.
- On release we `finish()` → transcribe in another `spawn_blocking` → emit `voice://final` event carrying `{text: String}` → frontend injects into the input box and triggers the existing send flow.

- [ ] **Step 1: Add feature + dep**

Edit `crates/coati-desktop/Cargo.toml`:

```toml
[features]
default = []
desktop = [
    "dep:tauri",
    "dep:tauri-plugin-global-shortcut",
]
voice = [
    "desktop",
    "dep:coati-voice",
]

[dependencies]
# ... existing ...
coati-voice = { path = "../coati-voice", optional = true }
```

Update `[[bin]] required-features`:

```toml
[[bin]]
name = "coati-desktop"
path = "src/main.rs"
required-features = ["desktop"]
```

(Keep required-features as `["desktop"]`, not `["voice"]` — voice is opt-in on top.)

- [ ] **Step 2: Write voice.rs state machine**

```rust
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
            return Ok(()); // already recording
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

        let samples = tokio::task::spawn_blocking(move || ptt.finish()).await??;
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
    use crate::AppState;
    let state = app.state::<AppState>();
    let cfg = state.config.voice.clone().unwrap_or_default();
    (cfg.model, cfg.language)
}
```

Note: `AppState` must expose `config: Arc<Config>` already (established in Phase 3). If `pub` is missing, widen visibility.

- [ ] **Step 3: Extend shortcut.rs to route two hotkeys**

Read `crates/coati-desktop/src/shortcut.rs` first, then replace the body of `register` with logic that:

1. Parses both the chat hotkey (from `[desktop] hotkey`) and, if voice is enabled, the voice hotkey (from `[voice] hotkey`).
2. Registers both via `gs.on_shortcut(shortcut, handler)` *or* uses `on_all_shortcuts` with a match.
3. On chat hotkey: `ShortcutState::Pressed` → existing `toggle_main`.
4. On voice hotkey: `ShortcutState::Pressed` → `VoiceState::on_press`; `ShortcutState::Released` → `VoiceState::on_release`.

Concrete shape (drop-in replacement):

```rust
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub fn register<R: Runtime>(app: &AppHandle<R>, chat_hotkey: &str) -> anyhow::Result<()> {
    #[cfg(feature = "voice")]
    let voice_hotkey: Option<String> = {
        let state = app.state::<crate::AppState>();
        state
            .config
            .voice
            .as_ref()
            .filter(|v| v.enabled)
            .map(|v| v.hotkey.clone())
    };
    #[cfg(not(feature = "voice"))]
    let voice_hotkey: Option<String> = None;

    let chat_hk = chat_hotkey.to_string();
    let voice_hk = voice_hotkey.clone();

    let app_clone = app.clone();
    app.global_shortcut().on_shortcut(chat_hotkey, move |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            crate::tray::toggle_main(app);
        }
    })?;

    #[cfg(feature = "voice")]
    if let Some(vhk) = voice_hk {
        app.global_shortcut().on_shortcut(vhk.as_str(), move |app, _shortcut, event| {
            let handle = app.clone();
            let state = handle.state::<crate::AppState>();
            let (model, lang) = (
                state.config.voice.clone().unwrap_or_default().model,
                state.config.voice.clone().unwrap_or_default().language,
            );
            let voice = handle.state::<crate::voice::VoiceState>().inner.clone();
            // We can't hold locks across await in this sync callback, so spawn.
            match event.state() {
                ShortcutState::Pressed => {
                    let h2 = handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let vs = h2.state::<crate::voice::VoiceState>();
                        let _ = vs.on_press(&h2, &model, &lang).await;
                    });
                }
                ShortcutState::Released => {
                    let h2 = handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let vs = h2.state::<crate::voice::VoiceState>();
                        let _ = vs.on_release(&h2).await;
                    });
                }
            }
        })?;
    }

    Ok(())
}
```

Subagent note: the `voice = handle.state::<...>().inner.clone()` line is illustrative — if it doesn't fit the chosen `VoiceState` shape, drop it; the `h2.state::<VoiceState>()` calls inside the `spawn` are the ones that actually matter.

- [ ] **Step 4: Wire in main.rs**

Edit `crates/coati-desktop/src/main.rs`:

```rust
#[cfg(feature = "voice")]
mod voice;
```

In the `.setup(|app| ...)` block, after `tray::init(app)?;`:

```rust
#[cfg(feature = "voice")]
app.manage(voice::VoiceState::default());
```

- [ ] **Step 5: Add frontend banner**

Edit `crates/coati-desktop/dist/index.html` — add inside `<main id="main">` before `<div id="messages">`:

```html
<div id="rec-banner" hidden>
  <span class="dot"></span> Listening… release F9 to send
</div>
```

CSS (append to existing stylesheet in index.html `<style>` block):

```css
#rec-banner {
  background: #6AA07F;
  color: #1A130E;
  padding: 6px 10px;
  font-family: "IBM Plex Mono", monospace;
  font-size: 12px;
  display: flex;
  align-items: center;
  gap: 8px;
}
#rec-banner .dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #E67347;
  animation: pulse 1s infinite ease-in-out;
}
@keyframes pulse {
  0%, 100% { opacity: 0.4; }
  50% { opacity: 1; }
}
```

- [ ] **Step 6: Extend app.js**

Append to `crates/coati-desktop/dist/app.js`:

```js
(() => {
  const banner = document.getElementById("rec-banner");
  const input = document.getElementById("input");
  const sendBtn = document.getElementById("send-btn");

  if (!window.__TAURI__ || !banner) return;
  const { listen } = window.__TAURI__.event;

  listen("voice://recording", () => {
    banner.hidden = false;
    banner.textContent = "";
    const dot = document.createElement("span");
    dot.className = "dot";
    banner.appendChild(dot);
    banner.appendChild(document.createTextNode(" Listening… release F9 to send"));
    if (input) input.disabled = true;
  });

  listen("voice://transcribing", () => {
    banner.hidden = false;
    while (banner.firstChild) banner.removeChild(banner.firstChild);
    const dot = document.createElement("span");
    dot.className = "dot";
    banner.appendChild(dot);
    banner.appendChild(document.createTextNode(" Transcribing…"));
  });

  listen("voice://idle", () => {
    banner.hidden = true;
    if (input) input.disabled = false;
  });

  listen("voice://final", (event) => {
    const text = (event.payload && event.payload.text) || "";
    if (!text.trim()) return;
    if (input) {
      input.disabled = false;
      input.value = text;
      if (sendBtn) sendBtn.click();
    }
  });
})();
```

Subagent note: the element ids (`input`, `send-btn`, `rec-banner`) must match what Phase 3 shipped. Read `dist/index.html` first and reconcile.

- [ ] **Step 7: Verify build**

Run: `cargo check -p coati-desktop --features voice`
Expected: compiles. (A fresh machine needs `libasound2-dev clang pkg-config` — CI handles this in Task 9.)

- [ ] **Step 8: Commit**

```bash
git add crates/coati-desktop/src/voice.rs \
        crates/coati-desktop/src/main.rs \
        crates/coati-desktop/src/shortcut.rs \
        crates/coati-desktop/Cargo.toml \
        crates/coati-desktop/dist/index.html \
        crates/coati-desktop/dist/app.js
git commit -m "feat(desktop): F9 push-to-talk voice integration"
```

---

## Task 8: CI job for coati-voice (unit tests) + extend desktop job (voice build)

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add a voice job**

Open `.github/workflows/ci.yml`. Add after the existing `desktop` job:

```yaml
  voice:
    name: voice (coati-voice crate)
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - name: Install system deps
        run: |
          sudo apt-get update
          sudo apt-get install -y libasound2-dev clang pkg-config libclang-dev cmake
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: "1.82"
      - uses: Swatinem/rust-cache@v2
      - name: cargo test
        run: cargo test -p coati-voice
      - name: cargo clippy
        run: cargo clippy -p coati-voice -- -D warnings
```

Extend the existing `desktop` job's "Install system deps" step to include `libasound2-dev clang pkg-config libclang-dev cmake`, and add a second build step:

```yaml
      - name: Build with voice
        run: cargo build -p coati-desktop --features voice
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: coati-voice test job + desktop voice build"
```

- [ ] **Step 3: Push and wait for CI**

```bash
git push
gh run list --limit 1
```

If red, diagnose via `gh run view --log-failed <id>` and fix before proceeding.

---

## Task 9: Install script + README voice section

**Files:**
- Modify: `scripts/install-desktop.sh` — accept `--with-voice`
- Modify: `README.md` — add Voice section
- Modify: `shell/install.sh` — mention voice install hint
- Modify: `ROADMAP.md` — mark Phase 4 shipped
- Modify: `CLAUDE.md` — append Phase 4 line

- [ ] **Step 1: Patch install-desktop.sh**

Read the current script, then at the top add flag parsing:

```bash
FEATURES="desktop"
for arg in "$@"; do
  case "$arg" in
    --with-voice) FEATURES="desktop,voice" ;;
  esac
done
```

And replace the cargo build invocation with `cargo build --release -p coati-desktop --features "$FEATURES"`. Echo the enabled features.

- [ ] **Step 2: Append README Voice section**

Append to `README.md`:

```markdown

## Voice (push-to-talk, optional)

Install with voice enabled:

```sh
./scripts/install-desktop.sh --with-voice
```

First run, download a model:

```sh
coati voice setup              # base.en (~148 MB, recommended)
coati voice setup --model tiny.en  # smaller, faster, less accurate
```

Then hold **F9** in the chat window to talk, release to send. Audio never leaves your machine — the only network call the voice subsystem makes is the model download above.

Change the hotkey via `[voice] hotkey = "F9"` in `~/.config/coati/config.toml`. Restart Coati after editing.
```

- [ ] **Step 3: Update ROADMAP + CLAUDE.md**

`ROADMAP.md`: change Phase 4's header to `## Phase 4 — Voice (SHIPPED 2026-04-18)` and check the boxes.

`CLAUDE.md`: add after the Phase 2 shipped line:

```markdown
**Phase 4 (push-to-talk voice) shipped 2026-04-18** — F9 hold-to-talk in desktop chat, whisper-rs local transcription, model downloads SHA-256 verified. Next: Phase 5 (packaging + launch).
```

- [ ] **Step 4: Commit**

```bash
git add scripts/install-desktop.sh README.md ROADMAP.md CLAUDE.md shell/install.sh
git commit -m "docs(voice): README voice section, installer flag, roadmap update"
```

---

## Task 10: Dogfood smoke test

**Files:**
- Create: `tests/e2e/voice.sh`

- [ ] **Step 1: Write the script**

```bash
#!/usr/bin/env bash
# tests/e2e/voice.sh — dogfood test for voice pipeline.
# Requires: coati built with --features voice, a base.en or tiny.en model installed,
# and `sox` or `ffmpeg` on PATH for synthesizing a silent WAV.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="$ROOT/target/release/coati"
[ -x "$BIN" ] || BIN="$ROOT/target/debug/coati"

if [ ! -x "$BIN" ]; then
  echo "error: coati binary not found — run 'cargo build --features voice'"
  exit 1
fi

WAV="$(mktemp --suffix=.wav)"
trap 'rm -f "$WAV"' EXIT

# Synthesize 1 second of silence at 16kHz mono.
if command -v sox >/dev/null 2>&1; then
  sox -n -r 16000 -c 1 "$WAV" trim 0.0 1.0
elif command -v ffmpeg >/dev/null 2>&1; then
  ffmpeg -y -f lavfi -i anullsrc=r=16000:cl=mono -t 1 -acodec pcm_s16le "$WAV" -loglevel error
else
  echo "skip: need sox or ffmpeg to synthesize WAV"
  exit 0
fi

MODEL="${COATI_VOICE_MODEL:-base.en}"
echo "Transcribing $WAV with model=$MODEL"
OUT="$("$BIN" voice transcribe "$WAV" --model "$MODEL")"
echo "Output: '$OUT'"

# Silence should produce empty or very short output.
LEN=${#OUT}
if [ "$LEN" -gt 200 ]; then
  echo "FAIL: unexpected long transcript on silence ($LEN chars)"
  exit 1
fi
echo "PASS"
```

Make executable: `chmod +x tests/e2e/voice.sh`.

- [ ] **Step 2: Run locally (controller, outside subagent)**

```bash
cargo build --features voice -p coati-cli
./target/debug/coati voice setup --model tiny.en --yes
./tests/e2e/voice.sh
```

If PASS, commit. If it fails because whisper-rs can't build on the user's machine, capture the error and file it as an open item in the commit message — do not block the phase on first-machine build issues since CI covers the Linux distro path.

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/voice.sh
git commit -m "test(voice): e2e smoke script for transcribe on silence"
```

---

## Task 11: Tag v0.0.4-phase4

**Controller task, not a subagent.**

- [ ] **Step 1: Verify clean tree + green CI**

```bash
git status --short
gh run list --limit 1
```

Tree must be empty. Last CI run must be ✅.

- [ ] **Step 2: Tag + push**

```bash
git tag -a v0.0.4-phase4 -m "Phase 4: push-to-talk voice (F9 hold-to-talk, whisper-rs local transcription)"
git push origin v0.0.4-phase4
```

- [ ] **Step 3: Update memory**

Edit `/home/marche/.claude/projects/-home-marche/memory/project_coati.md` — add a Phase 4 SHIPPED block at the top (same shape as the Phase 3 block already there), noting:
- whisper-rs 0.13, cpal 0.15
- base.en default, tiny.en optional; downloaded on first run with SHA-256 verify
- F9 hold-to-talk in desktop; configurable via `[voice]` section
- Audio stays local; only network call is the accepted model download
- Known open items: tray icon pulse is webview-only (native tray icon swap = v1.0)

---

## Self-Review Checklist (controller runs before dispatching Task 1)

1. **Spec coverage:** every Phase 4 ROADMAP checkpoint → Tasks 7 (hotkey-to-response), 10 (latency under 2s is a hand-measure, not automated), 8+9 (no network at runtime — covered by the model-download-only audit). Bundle whisper `small.en` → overridden to base.en per user decision; documented in README.
2. **Placeholder scan:** no TBD / TODO / "similar to Task N" left in-body. Live WAV fixture is *generated* at test time, not a checked-in binary — explicit.
3. **Type consistency:** `PushToTalk::start/finish`, `Transcriber::new/with_language/transcribe`, `ModelSpec { name, url, sha256, size_mb }`, `VoiceConfig { enabled, hotkey, model, language }` — names stable across all tasks.
4. **Feature gates:** `coati-cli` default build does NOT pull coati-voice (Task 3 step 7); `coati-desktop` default build does NOT pull voice (Task 7 Cargo.toml shape); `coati-core` never depends on coati-voice.
5. **Rust 1.82 pin:** whisper-rs 0.13, cpal 0.15, hound 3.5, sha2 0.10, futures-util 0.3 — all compatible with edition 2021 / 1.82 as of 2026-04. If a `=` pin is needed, add it in the task where it first bites.

---

## Execution

Roll straight into subagent-driven-development. No confirmation checkpoint between tasks — fix-forward on any red CI per project convention.
