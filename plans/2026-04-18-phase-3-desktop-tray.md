# Coati Phase 3: Desktop Tray + Chat Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a Tauri 2.x tray application that summons a chat window via global hotkey, streams model responses token-by-token, persists history to SQLite, and mirrors the shell plugin's confirm-before-sudo UX for any `propose` intents invoked from the chat. All traffic stays on the Unix socket daemon from Phase 1. The desktop app is an optional surface — the CLI and shell plugins from Phases 1–2 must keep working for users who never install it.

**Architecture:** Four layers, cleanly separated.

1. **IPC protocol extension** in `coati-core/src/ipc.rs`: add `Request::AskStream` + `Response::Chunk`/`Response::StreamEnd` frames so every surface (desktop today, voice later) streams through the same daemon. Keep `Request::Ask` (non-streaming) for CLI one-shot use.
2. **Daemon streaming handler** in `coati-cli/src/cmd_serve.rs` consumes `AskStream`, calls a new `OllamaClient::complete_stream()`, and writes newline-delimited JSON frames back on the same socket connection until `StreamEnd`.
3. **Persistence layer** `coati-core/src/history.rs` — `rusqlite`-backed repo with `conversations` + `messages` tables at `~/.local/share/coati/history.db`. Shared, not Tauri-specific, so future surfaces (web, TUI) can read the same history.
4. **Desktop crate** `crates/coati-desktop/` — Tauri 2.x. Rust backend registers commands (`list_models`, `send_stream`, `create_conversation`, `list_conversations`, `load_conversation`, `run_proposal`, `get_settings`, `set_settings`) and a global shortcut. Vanilla JS frontend (no bundler needed for MVP) with bundled IBM Plex fonts. Strict CSP blocks all remote resources. Tray icon + menu (Open Chat / Toggle Listening disabled placeholder / Settings / Quit).

**Design decisions locked in (answers to the spec's open questions):**

1. **Streaming protocol:** extend the Unix socket IPC with `Request::AskStream` → stream of `Response::Chunk { delta }` frames followed by one `Response::StreamEnd { full_content }`. Chosen over "Tauri calls ollama directly" because (a) it keeps the daemon as the only process talking to the LLM, which makes future remote-model toggles one place to change, (b) voice in Phase 4 gets streaming for free, (c) request/reply stays one-shot; streaming is a new, explicit verb.
2. **History schema:** two tables. `conversations (id TEXT PRIMARY KEY, title TEXT, created_at INTEGER, updated_at INTEGER, model TEXT)` and `messages (id TEXT PRIMARY KEY, conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE, role TEXT, content TEXT, model TEXT, created_at INTEGER)`. Resume = pass `conversation_id` to `AskStream`; daemon loads prior messages and prepends them to the prompt.
3. **Tray menu:** `Open Chat` (shows/focuses window) / `Toggle Listening` (disabled + labeled `(Phase 4)`) / `Settings` / `Quit`. Clicking the tray icon toggles the chat window.
4. **Settings:** new `[desktop]` section in the existing `~/.config/coati/config.toml`, fields `hotkey = "Ctrl+Space"`, `theme = "coati"`, `window_width = 480`, `window_height = 640`, `history_enabled = true`. One config file for CLI + desktop — no split.

**Tech Stack:** Tauri 2.1, `tauri-plugin-global-shortcut` 2.x, `tauri-plugin-tray-icon` (built into tauri 2.x), `rusqlite` 0.32 (bundled feature), `uuid` 1.x, vanilla JS/HTML/CSS for the frontend (no build step), bundled IBM Plex Serif/Mono/Sans WOFF2 files.

---

## File Structure

```
/home/marche/coati/
├── Cargo.toml                                # MODIFIED — add coati-desktop member + workspace deps
├── crates/
│   ├── coati-core/src/
│   │   ├── ipc.rs                            # MODIFIED — AskStream, Chunk, StreamEnd
│   │   ├── llm.rs                            # MODIFIED — OllamaClient::complete_stream()
│   │   ├── history.rs                        # NEW — SQLite repo (conversations, messages)
│   │   ├── config.rs                         # MODIFIED — DesktopConfig section
│   │   └── lib.rs                            # MODIFIED — re-export history
│   ├── coati-cli/src/
│   │   └── cmd_serve.rs                      # MODIFIED — handle AskStream, write ND-JSON frames
│   └── coati-desktop/                        # NEW CRATE
│       ├── Cargo.toml
│       ├── tauri.conf.json
│       ├── build.rs
│       ├── icons/
│       │   ├── tray-16.png                   # simplified glyph
│       │   ├── tray-32.png
│       │   ├── icon-128.png
│       │   └── icon-512.png
│       ├── src/
│       │   ├── main.rs                       # entry + setup + tray + shortcut
│       │   ├── commands.rs                   # #[tauri::command] handlers
│       │   ├── stream.rs                     # client: talks AskStream to daemon socket
│       │   ├── tray.rs                       # tray menu construction
│       │   └── shortcut.rs                   # global hotkey registration + toggle
│       └── dist/                             # static frontend
│           ├── index.html
│           ├── app.js
│           ├── app.css
│           └── fonts/
│               ├── IBMPlexSerif-Italic.woff2
│               ├── IBMPlexMono-Regular.woff2
│               └── IBMPlexSans-Regular.woff2
└── .github/workflows/ci.yml                  # MODIFIED — new `desktop` job
```

**Decomposition principle:** the daemon owns streaming + history because every surface needs them; Tauri owns UI + tray + hotkey because those are desktop-specific. The desktop crate is opt-in — removing it from `workspace.members` must leave everything else building.

Task index:
- **Task 1:** Workspace scaffold for `coati-desktop` crate (empty, compiles, excluded from default build)
- **Task 2:** Extend IPC with `AskStream` / `Chunk` / `StreamEnd`
- **Task 3:** `OllamaClient::complete_stream()` — NDJSON chunk iterator
- **Task 4:** Daemon handler for `AskStream` (write ND-JSON frames to socket)
- **Task 5:** `DesktopConfig` section in `config.toml`
- **Task 6:** SQLite history repo (`conversations`, `messages`)
- **Task 7:** Tauri backend scaffold (`main.rs`, `tauri.conf.json`, CSP lockdown)
- **Task 8:** Tauri commands — `list_models`, `list_conversations`, `load_conversation`, `create_conversation`
- **Task 9:** Tauri command — `send_stream` (bridges frontend to daemon)
- **Task 10:** Tauri command — `run_proposal` with confirm-before-sudo
- **Task 11:** Frontend scaffold (HTML/CSS/JS, bundled IBM Plex, CSP-strict)
- **Task 12:** Frontend chat UI + streaming display
- **Task 13:** Frontend model selector + conversation sidebar
- **Task 14:** Tray icon + tray menu
- **Task 15:** Global hotkey + window toggle
- **Task 16:** Settings window (hotkey / theme / window size)
- **Task 17:** README section + `install.sh` opt-in block for desktop
- **Task 18:** CI job for desktop build

Detailed task bodies follow.

---

## Task 1: Workspace scaffold for `coati-desktop` crate

**Files:**
- Create: `crates/coati-desktop/Cargo.toml`
- Create: `crates/coati-desktop/src/main.rs`
- Create: `crates/coati-desktop/build.rs`
- Create: `crates/coati-desktop/tauri.conf.json`
- Modify: `Cargo.toml` (workspace members + shared deps)

- [ ] **Step 1: Add Tauri-related entries to workspace `Cargo.toml`**

Replace the `[workspace.dependencies]` block in `/home/marche/coati/Cargo.toml` with:

```toml
[workspace.dependencies]
tokio = { version = "1.40", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
clap = { version = "=4.5.45", features = ["derive"] }
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1.0"
rig-core = "0.35"
rusqlite = { version = "0.32", features = ["bundled"] }
uuid = { version = "1.11", features = ["v4"] }
tauri = { version = "2.1", features = ["tray-icon"] }
tauri-build = "2.0"
tauri-plugin-global-shortcut = "2.0"
```

Change the `members` line to:

```toml
members = ["crates/coati-core", "crates/coati-tools", "crates/coati-cli", "crates/coati-hw", "crates/coati-desktop"]
```

- [ ] **Step 2: Create `crates/coati-desktop/Cargo.toml`**

```toml
[package]
name = "coati-desktop"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true

[build-dependencies]
tauri-build = { workspace = true }

[dependencies]
coati-core = { path = "../coati-core" }
tauri = { workspace = true }
tauri-plugin-global-shortcut = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

- [ ] **Step 3: Create minimal `crates/coati-desktop/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 4: Create `crates/coati-desktop/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Coati",
  "version": "0.0.3",
  "identifier": "sh.coati.desktop",
  "build": {
    "frontendDist": "dist"
  },
  "app": {
    "windows": [
      {
        "title": "Coati",
        "label": "main",
        "width": 480,
        "height": 640,
        "visible": false,
        "resizable": true,
        "decorations": true
      }
    ],
    "security": {
      "csp": "default-src 'self'; img-src 'self' data:; font-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["deb"],
    "icon": ["icons/icon-128.png", "icons/icon-512.png"],
    "category": "Utility"
  }
}
```

- [ ] **Step 5: Create minimal `crates/coati-desktop/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: Place placeholder icons**

For the build to succeed Tauri needs at least the two icons declared above. Use the existing `brand.html` SVG rendered to PNG via any image tool. Acceptable fallback for Task 1 only — a blank 128×128 PNG and 512×512 PNG so the build completes. Real icons arrive in Task 14.

```bash
mkdir -p /home/marche/coati/crates/coati-desktop/icons
cd /home/marche/coati/crates/coati-desktop/icons
# 1x1 transparent PNG; tauri just needs any valid PNG for the scaffold.
printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\rIDATx\x9cc\xf8\xff\xff?\x00\x05\xfe\x02\xfe\xdc\xcc\x59\xe7\x00\x00\x00\x00IEND\xaeB`\x82' > icon-128.png
cp icon-128.png icon-512.png
cp icon-128.png tray-16.png
cp icon-128.png tray-32.png
```

- [ ] **Step 7: Create frontend placeholder `crates/coati-desktop/dist/index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>Coati</title>
  </head>
  <body>
    <p>Coati desktop scaffold.</p>
  </body>
</html>
```

- [ ] **Step 8: Run `cargo build -p coati-desktop`**

Expected: compiles. On a headless machine without the system libs (`libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`) `cargo build -p coati-desktop` will fail — that's acceptable, the CI job in Task 18 installs them. Locally: `sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`.

- [ ] **Step 9: Confirm the rest of the workspace still builds without desktop**

```bash
cargo build -p coati-cli
cargo build -p coati-core
cargo test -p coati-core
```

Expected: all pass. The CLI must never depend on Tauri.

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml crates/coati-desktop/
git commit -m "feat(desktop): scaffold coati-desktop crate with tauri 2.x

Adds a new workspace member that builds to a headless shell (no window
shown yet). Tauri config ships with a strict CSP that blocks remote
resources; icon placeholders will be replaced in Task 14."
```

---

## Task 2: Extend IPC with `AskStream` / `Chunk` / `StreamEnd`

**Files:**
- Modify: `crates/coati-core/src/ipc.rs`

- [ ] **Step 1: Write failing tests at the bottom of the `#[cfg(test)] mod tests` block in `crates/coati-core/src/ipc.rs`**

```rust
#[test]
fn serializes_ask_stream_request() {
    let req = Request::AskStream {
        question: "what is my disk usage".into(),
        conversation_id: Some("c-1".into()),
    };
    let s = serde_json::to_string(&req).unwrap();
    assert!(s.contains("\"type\":\"ask_stream\""));
    assert!(s.contains("\"conversation_id\":\"c-1\""));
}

#[test]
fn deserializes_chunk_response() {
    let s = r#"{"type":"chunk","delta":"hello"}"#;
    let r: Response = serde_json::from_str(s).unwrap();
    match r {
        Response::Chunk { delta } => assert_eq!(delta, "hello"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn deserializes_stream_end_response() {
    let s = r#"{"type":"stream_end","full_content":"hello world"}"#;
    let r: Response = serde_json::from_str(s).unwrap();
    match r {
        Response::StreamEnd { full_content } => assert_eq!(full_content, "hello world"),
        _ => panic!("wrong variant"),
    }
}
```

- [ ] **Step 2: Run `cargo test -p coati-core ipc::tests` → failures for the three new tests (variants not defined)**

- [ ] **Step 3: Add variants to `Request` and `Response` enums**

In `crates/coati-core/src/ipc.rs`, add to `pub enum Request`:

```rust
    AskStream {
        question: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        conversation_id: Option<String>,
    },
```

And to `pub enum Response`:

```rust
    Chunk { delta: String },
    StreamEnd { full_content: String },
```

- [ ] **Step 4: Run `cargo test -p coati-core ipc::tests` → all pass**

- [ ] **Step 5: Run `cargo clippy --workspace --all-targets -- -D warnings` → clean**

- [ ] **Step 6: Commit**

```bash
git add crates/coati-core/src/ipc.rs
git commit -m "feat(ipc): add AskStream request and Chunk/StreamEnd response

Prepares the daemon-socket protocol for newline-delimited streaming.
conversation_id lets the daemon resume history when Phase 3 history
persistence lands."
```

---

## Task 3: `OllamaClient::complete_stream()` — NDJSON chunk iterator

**Files:**
- Modify: `crates/coati-core/src/llm.rs`
- Modify: `crates/coati-core/Cargo.toml`

- [ ] **Step 1: Add `futures-util = "0.3"` to `[dependencies]` of `crates/coati-core/Cargo.toml` if not already there.**

- [ ] **Step 2: Add a streaming test module at the bottom of `crates/coati-core/src/llm.rs`**

Use a wiremock server to assert the streaming client decodes ollama's `{"message":{"content":"..."},"done":false}` NDJSON frames.

```rust
#[cfg(test)]
mod stream_tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn streams_chunks_until_done() {
        let server = MockServer::start().await;
        let body = concat!(
            r#"{"message":{"role":"assistant","content":"hel"},"done":false}"#, "\n",
            r#"{"message":{"role":"assistant","content":"lo"},"done":false}"#, "\n",
            r#"{"message":{"role":"assistant","content":""},"done":true}"#, "\n",
        );
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "gemma4".into());
        let mut chunks = vec![];
        let full = client
            .complete_stream(
                vec![ChatMessage::user("hi")],
                |c| chunks.push(c.to_string()),
            )
            .await
            .unwrap();

        assert_eq!(chunks, vec!["hel", "lo"]);
        assert_eq!(full, "hello");
    }
}
```

- [ ] **Step 3: Run `cargo test -p coati-core stream_tests` → compile fails (no `complete_stream`)**

- [ ] **Step 4: Implement `complete_stream` on `impl OllamaClient`**

Add at the top of `crates/coati-core/src/llm.rs`:

```rust
use futures_util::StreamExt;
```

Add the method (place after the existing `complete_json` method):

```rust
impl OllamaClient {
    pub async fn complete_stream<F>(
        &self,
        messages: Vec<ChatMessage>,
        mut on_chunk: F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(&str),
    {
        #[derive(serde::Serialize)]
        struct Req<'a> {
            model: &'a str,
            messages: &'a [ChatMessage],
            stream: bool,
        }
        #[derive(serde::Deserialize)]
        struct Line {
            message: Option<Msg>,
            done: bool,
        }
        #[derive(serde::Deserialize)]
        struct Msg { content: String }

        let url = format!("{}/api/chat", self.endpoint);
        let req = Req { model: &self.model, messages: &messages, stream: true };
        let resp = self
            .http
            .post(url)
            .json(&req)
            .send()
            .await?
            .error_for_status()?;

        let mut stream = resp.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();
        let mut full = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            buf.extend_from_slice(&bytes);
            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let line = buf.drain(..=pos).collect::<Vec<u8>>();
                let line = std::str::from_utf8(&line[..line.len() - 1])?.trim();
                if line.is_empty() { continue; }
                let parsed: Line = serde_json::from_str(line)?;
                if let Some(m) = parsed.message {
                    if !m.content.is_empty() {
                        on_chunk(&m.content);
                        full.push_str(&m.content);
                    }
                }
                if parsed.done {
                    return Ok(full);
                }
            }
        }
        Ok(full)
    }
}
```

- [ ] **Step 5: Run `cargo test -p coati-core stream_tests::streams_chunks_until_done` → passes**

- [ ] **Step 6: Run `cargo clippy --workspace --all-targets -- -D warnings` → clean**

- [ ] **Step 7: Commit**

```bash
git add crates/coati-core/Cargo.toml crates/coati-core/src/llm.rs
git commit -m "feat(llm): add OllamaClient::complete_stream for NDJSON streaming

Callbacks fire on each non-empty content delta; returns the full
concatenated string once the server sends done=true. Uses futures-util
for the byte-stream line buffer."
```

---

## Task 4: Daemon handler for `AskStream`

**Files:**
- Modify: `crates/coati-cli/src/cmd_serve.rs`
- Create: `crates/coati-cli/tests/serve_stream.rs`

- [ ] **Step 1: Write a hermetic fake-daemon test at `crates/coati-cli/tests/serve_stream.rs`**

```rust
//! Locks the wire format: AskStream request in → stream of Chunk frames → StreamEnd.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use coati_core::ipc::{Request, Response};

fn sock_path() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("coati-stream-test-{}.sock", std::process::id()));
    p
}

#[test]
fn ask_stream_returns_frames_and_end() {
    let sp = sock_path();
    let _ = std::fs::remove_file(&sp);
    let sp_bg = sp.clone();

    thread::spawn(move || {
        let listener = UnixListener::bind(&sp_bg).unwrap();
        for stream in listener.incoming() {
            let mut s = stream.unwrap();
            let mut reader = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let _req: Request = serde_json::from_str(line.trim()).unwrap();
            let c1 = serde_json::to_string(&Response::Chunk { delta: "hel".into() }).unwrap();
            let c2 = serde_json::to_string(&Response::Chunk { delta: "lo".into() }).unwrap();
            let end = serde_json::to_string(&Response::StreamEnd { full_content: "hello".into() }).unwrap();
            writeln!(s, "{c1}").unwrap();
            writeln!(s, "{c2}").unwrap();
            writeln!(s, "{end}").unwrap();
            return;
        }
    });

    thread::sleep(Duration::from_millis(50));

    let mut client = UnixStream::connect(&sp).unwrap();
    let req = Request::AskStream { question: "hi".into(), conversation_id: None };
    writeln!(client, "{}", serde_json::to_string(&req).unwrap()).unwrap();

    let reader = BufReader::new(client);
    let mut deltas = vec![];
    let mut full = None;
    for line in reader.lines() {
        let line = line.unwrap();
        let r: Response = serde_json::from_str(&line).unwrap();
        match r {
            Response::Chunk { delta } => deltas.push(delta),
            Response::StreamEnd { full_content } => { full = Some(full_content); break; }
            _ => panic!("unexpected frame"),
        }
    }

    assert_eq!(deltas, vec!["hel", "lo"]);
    assert_eq!(full.as_deref(), Some("hello"));
    let _ = std::fs::remove_file(&sp);
}
```

- [ ] **Step 2: Run `cargo test -p coati-cli --test serve_stream` → passes (locks the wire format before wiring the real daemon).**

- [ ] **Step 3: Add a streaming branch to the daemon's request dispatcher**

In `crates/coati-cli/src/cmd_serve.rs`, locate the `match request { ... }` block (where `Request::Ask`, `Request::Propose`, etc. are handled). Add a branch before the wildcard:

```rust
Request::AskStream { question, conversation_id } => {
    use std::io::Write;
    use coati_core::llm::{ChatMessage, OllamaClient};

    let llm = OllamaClient::new(cfg.llm.endpoint.clone(), cfg.llm.model.clone());

    // Load prior messages if a conversation is named.
    let mut messages: Vec<ChatMessage> = vec![];
    if let Some(cid) = conversation_id.as_ref() {
        if let Ok(history) = coati_core::history::HistoryRepo::open_default() {
            for m in history.messages(cid)? {
                messages.push(if m.role == "user" {
                    ChatMessage::user(m.content)
                } else {
                    ChatMessage::assistant(m.content)
                });
            }
        }
    }
    messages.push(ChatMessage::user(question.clone()));

    let mut full = String::new();
    let res = llm
        .complete_stream(messages, |delta| {
            let frame = serde_json::to_string(&Response::Chunk { delta: delta.into() }).unwrap();
            let _ = writeln!(conn, "{frame}");
            full.push_str(delta);
        })
        .await;

    match res {
        Ok(_) => {
            let end = serde_json::to_string(&Response::StreamEnd { full_content: full.clone() }).unwrap();
            let _ = writeln!(conn, "{end}");

            if let Some(cid) = conversation_id.as_ref() {
                if let Ok(history) = coati_core::history::HistoryRepo::open_default() {
                    let _ = history.append_message(cid, "user", &question, &cfg.llm.model);
                    let _ = history.append_message(cid, "assistant", &full, &cfg.llm.model);
                }
            }
        }
        Err(e) => {
            let err = serde_json::to_string(&Response::Error { message: e.to_string() }).unwrap();
            let _ = writeln!(conn, "{err}");
        }
    }
}
```

**Note:** the existing daemon writes one JSON reply per connection. Streaming breaks that shape, so the connection MUST stay open across multiple `writeln!`s for this branch only. Ensure the connection is not closed until after the final `StreamEnd` frame is flushed.

- [ ] **Step 4: Run `cargo test --workspace` → all pass**

- [ ] **Step 5: Manual smoke — one terminal `cargo run -p coati-cli -- serve`, another:**

```bash
python3 -c '
import socket, json
s = socket.socket(socket.AF_UNIX)
s.connect("/home/marche/.cache/coati/agent.sock")
s.send(json.dumps({"type":"ask_stream","question":"say hi"}).encode() + b"\n")
f = s.makefile("rb")
for line in f:
    print(line.decode().rstrip())
'
```

Expected: a sequence of `{"type":"chunk","delta":"..."}` lines ending with `{"type":"stream_end","full_content":"..."}`.

- [ ] **Step 6: Commit**

```bash
git add crates/coati-cli/
git commit -m "feat(serve): handle AskStream, write ND-JSON frames over unix socket

Loads prior messages from history when a conversation_id is supplied,
streams deltas as Chunk frames, closes with a single StreamEnd, then
appends the user turn and full assistant reply to history."
```

---

## Task 5: `DesktopConfig` section in `config.toml`

**Files:**
- Modify: `crates/coati-core/src/config.rs`

- [ ] **Step 1: Add tests in the existing `#[cfg(test)] mod tests` block**

```rust
#[test]
fn parses_desktop_section() {
    let toml_str = r#"
        [llm]
        provider = "ollama"
        endpoint = "http://localhost:11434"
        model = "gemma4"
        [tools]
        enabled = ["exec"]
        [desktop]
        hotkey = "Ctrl+Alt+Space"
        theme = "coati"
        window_width = 520
        window_height = 700
        history_enabled = true
    "#;
    let c: Config = toml::from_str(toml_str).unwrap();
    let d = c.desktop.expect("desktop section");
    assert_eq!(d.hotkey, "Ctrl+Alt+Space");
    assert_eq!(d.window_width, 520);
    assert!(d.history_enabled);
}

#[test]
fn default_desktop_is_sensible() {
    let d = DesktopConfig::default();
    assert_eq!(d.hotkey, "Ctrl+Space");
    assert_eq!(d.window_width, 480);
    assert_eq!(d.window_height, 640);
    assert!(d.history_enabled);
}
```

- [ ] **Step 2: Run `cargo test -p coati-core config::tests::parses_desktop_section` → fails (no `desktop` field, no `DesktopConfig`)**

- [ ] **Step 3: Add struct + field in `crates/coati-core/src/config.rs`**

Add to the top of the file (after `use` lines):

```rust
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DesktopConfig {
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    #[serde(default = "default_true")]
    pub history_enabled: bool,
}

fn default_hotkey() -> String { "Ctrl+Space".into() }
fn default_theme() -> String { "coati".into() }
fn default_window_width() -> u32 { 480 }
fn default_window_height() -> u32 { 640 }
fn default_true() -> bool { true }

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            hotkey: default_hotkey(),
            theme: default_theme(),
            window_width: default_window_width(),
            window_height: default_window_height(),
            history_enabled: default_true(),
        }
    }
}
```

Add field to `Config`:

```rust
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub llm: LlmConfig,
    pub tools: ToolsConfig,
    #[serde(default)]
    pub desktop: Option<DesktopConfig>,
}
```

Update `Default for Config`:

```rust
impl Default for Config {
    fn default() -> Self {
        Self {
            llm: LlmConfig {
                provider: "ollama".into(),
                endpoint: "http://localhost:11434".into(),
                model: "gemma4".into(),
            },
            tools: ToolsConfig {
                enabled: vec!["exec", "read_file", "list_dir", "query_logs", "explain_error"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
            },
            desktop: Some(DesktopConfig::default()),
        }
    }
}
```

- [ ] **Step 4: Run `cargo test -p coati-core config::tests` → all pass**

- [ ] **Step 5: Commit**

```bash
git add crates/coati-core/src/config.rs
git commit -m "feat(config): add [desktop] section with hotkey/theme/window dimensions

Keeps one config file for CLI + desktop. Existing configs without a
[desktop] section still parse (field is Option<DesktopConfig> with
serde default)."
```

---

## Task 6: SQLite history repo

**Files:**
- Create: `crates/coati-core/src/history.rs`
- Modify: `crates/coati-core/src/lib.rs` (re-export)
- Modify: `crates/coati-core/Cargo.toml` (add rusqlite, uuid, dirs)

- [ ] **Step 1: Add deps to `crates/coati-core/Cargo.toml` under `[dependencies]`**

```toml
rusqlite = { workspace = true }
uuid = { workspace = true }
dirs = "5.0"
```

(`dirs` may already exist; if so, skip.)

- [ ] **Step 2: Add `tempfile = "3.13.0"` to `[dev-dependencies]` of `crates/coati-core/Cargo.toml` if missing**

- [ ] **Step 3: Write the full history module at `crates/coati-core/src/history.rs`**

```rust
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct HistoryRepo {
    conn: Connection,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub model: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub model: String,
    pub created_at: i64,
}

impl HistoryRepo {
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from(".local/share"))
            .join("coati/history.db")
    }

    pub fn open_default() -> anyhow::Result<Self> {
        let path = Self::default_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::open(&path)
    }

    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS conversations (
              id TEXT PRIMARY KEY,
              title TEXT NOT NULL,
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL,
              model TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS messages (
              id TEXT PRIMARY KEY,
              conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
              role TEXT NOT NULL,
              content TEXT NOT NULL,
              model TEXT NOT NULL,
              created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_messages_conv_time
              ON messages(conversation_id, created_at);
            "#,
        )?;
        Ok(Self { conn })
    }

    pub fn create_conversation(&self, title: &str, model: &str) -> anyhow::Result<Conversation> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        self.conn.execute(
            "INSERT INTO conversations (id, title, created_at, updated_at, model) VALUES (?, ?, ?, ?, ?)",
            params![id, title, now, now, model],
        )?;
        Ok(Conversation { id, title: title.into(), created_at: now, updated_at: now, model: model.into() })
    }

    pub fn append_message(&self, conv_id: &str, role: &str, content: &str, model: &str) -> anyhow::Result<Message> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        self.conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, model, created_at) VALUES (?, ?, ?, ?, ?, ?)",
            params![id, conv_id, role, content, model, now],
        )?;
        self.conn.execute(
            "UPDATE conversations SET updated_at = ? WHERE id = ?",
            params![now, conv_id],
        )?;
        Ok(Message {
            id,
            conversation_id: conv_id.into(),
            role: role.into(),
            content: content.into(),
            model: model.into(),
            created_at: now,
        })
    }

    pub fn messages(&self, conv_id: &str) -> anyhow::Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, conversation_id, role, content, model, created_at
             FROM messages WHERE conversation_id = ? ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![conv_id], |r| {
            Ok(Message {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
                role: r.get(2)?,
                content: r.get(3)?,
                model: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn list_conversations(&self, limit: u32) -> anyhow::Result<Vec<Conversation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, created_at, updated_at, model FROM conversations
             ORDER BY updated_at DESC LIMIT ?",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            Ok(Conversation {
                id: r.get(0)?,
                title: r.get(1)?,
                created_at: r.get(2)?,
                updated_at: r.get(3)?,
                model: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn opens_fresh_db_and_creates_schema() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("h.db");
        let _repo = HistoryRepo::open(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn round_trips_conversation_and_messages() {
        let dir = TempDir::new().unwrap();
        let repo = HistoryRepo::open(&dir.path().join("h.db")).unwrap();
        let conv = repo.create_conversation("test", "gemma4").unwrap();
        repo.append_message(&conv.id, "user", "hi", "gemma4").unwrap();
        repo.append_message(&conv.id, "assistant", "hello", "gemma4").unwrap();
        let ms = repo.messages(&conv.id).unwrap();
        assert_eq!(ms.len(), 2);
        assert_eq!(ms[0].role, "user");
        assert_eq!(ms[1].content, "hello");
    }

    #[test]
    fn list_orders_by_updated_desc() {
        let dir = TempDir::new().unwrap();
        let repo = HistoryRepo::open(&dir.path().join("h.db")).unwrap();
        let a = repo.create_conversation("a", "gemma4").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        let b = repo.create_conversation("b", "gemma4").unwrap();
        let list = repo.list_conversations(10).unwrap();
        assert_eq!(list[0].id, b.id);
        assert_eq!(list[1].id, a.id);
    }
}
```

- [ ] **Step 4: Re-export in `crates/coati-core/src/lib.rs`**

Add:

```rust
pub mod history;
pub use history::{Conversation, HistoryRepo, Message};
```

- [ ] **Step 5: Run `cargo test -p coati-core history::tests` → 3/3 pass**

- [ ] **Step 6: Commit**

```bash
git add crates/coati-core/
git commit -m "feat(history): SQLite-backed conversation + message repository

Two tables (conversations, messages) with ON DELETE CASCADE and a
composite index for fast per-conversation lookup. HistoryRepo lives in
coati-core so every surface (desktop, voice, future TUI) shares one
schema. Default path is ~/.local/share/coati/history.db."
```

---

## Task 7: Tauri backend scaffold — entry, plugins, CSP

**Files:**
- Modify: `crates/coati-desktop/src/main.rs`
- Create: `crates/coati-desktop/src/commands.rs`
- Modify: `crates/coati-desktop/Cargo.toml` (add `dirs`)

- [ ] **Step 1: Add `dirs = "5.0"` to `crates/coati-desktop/Cargo.toml` dependencies**

- [ ] **Step 2: Rewrite `crates/coati-desktop/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

use coati_core::config::Config;

mod commands;

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
        .setup(|_app| {
            // window + tray + shortcut wiring lands in later tasks
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
```

- [ ] **Step 3: Create `crates/coati-desktop/src/commands.rs` with stubs**

```rust
use tauri::State;
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Serialize)]
pub struct ModelInfo { pub name: String, pub size: u64 }

#[tauri::command]
pub async fn list_models(_state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    Ok(vec![])
}

#[derive(Serialize, Deserialize)]
pub struct ConvRow { pub id: String, pub title: String, pub updated_at: i64 }

#[tauri::command]
pub async fn list_conversations(_state: State<'_, AppState>) -> Result<Vec<ConvRow>, String> {
    Ok(vec![])
}

#[derive(Serialize, Deserialize)]
pub struct MsgRow { pub role: String, pub content: String, pub created_at: i64 }

#[tauri::command]
pub async fn load_conversation(_state: State<'_, AppState>, _id: String) -> Result<Vec<MsgRow>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn create_conversation(_state: State<'_, AppState>, _title: String) -> Result<String, String> {
    Ok(String::new())
}

#[tauri::command]
pub async fn send_stream(_state: State<'_, AppState>, _question: String, _conversation_id: Option<String>) -> Result<(), String> {
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct ProposalPreview { pub command: String, pub reasoning: String, pub needs_sudo: bool }

#[tauri::command]
pub async fn run_proposal(_state: State<'_, AppState>, _command: String, _confirmed: bool) -> Result<String, String> {
    Ok(String::new())
}

#[derive(Serialize, Deserialize)]
pub struct Settings { pub hotkey: String, pub theme: String, pub window_width: u32, pub window_height: u32 }

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
```

Real implementations come in Tasks 8-10 and 16.

- [ ] **Step 4: Run `cargo test -p coati-desktop` → passes**

- [ ] **Step 5: Commit**

```bash
git add crates/coati-desktop/
git commit -m "feat(desktop): Tauri scaffold with AppState and command stubs

Wires tauri-plugin-global-shortcut and declares every invoke_handler
command upfront (list_models, send_stream, run_proposal, ...) so
frontend code can compile against stable signatures while the bodies
are filled in across Tasks 8-16. CSP in tauri.conf.json blocks all
remote origins."
```

---

## Task 8: Commands — `list_models`, `list_conversations`, `load_conversation`, `create_conversation`

**Files:**
- Modify: `crates/coati-desktop/src/commands.rs`
- Create: `crates/coati-desktop/src/ollama.rs`
- Modify: `crates/coati-desktop/Cargo.toml` (add `reqwest`, dev-dep `wiremock` + `tempfile`)

- [ ] **Step 1: Add deps to `crates/coati-desktop/Cargo.toml`**

```toml
[dependencies]
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }

[dev-dependencies]
wiremock = "0.5.22"
tempfile = "3.13.0"
```

- [ ] **Step 2: Create `crates/coati-desktop/src/ollama.rs`**

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct TagsResp { models: Vec<TagModel> }

#[derive(Deserialize)]
struct TagModel { name: String, size: u64 }

pub async fn list_installed(endpoint: &str) -> anyhow::Result<Vec<(String, u64)>> {
    let url = format!("{endpoint}/api/tags");
    let resp: TagsResp = reqwest::Client::new().get(url).send().await?.error_for_status()?.json().await?;
    Ok(resp.models.into_iter().map(|m| (m.name, m.size)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn parses_tags_response() {
        let s = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"models":[{"name":"gemma4","size":4000000000}]}"#,
            ))
            .mount(&s).await;
        let ms = list_installed(&s.uri()).await.unwrap();
        assert_eq!(ms[0].0, "gemma4");
        assert_eq!(ms[0].1, 4_000_000_000);
    }
}
```

- [ ] **Step 3: Declare the module in `main.rs`**

Add:

```rust
mod ollama;
```

- [ ] **Step 4: Add tests at the bottom of `crates/coati-desktop/src/commands.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn conv_row_serializes() {
        let r = ConvRow { id: "a".into(), title: "t".into(), updated_at: 1 };
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

pub async fn list_conversations_from(repo: &coati_core::history::HistoryRepo, limit: u32)
    -> Result<Vec<ConvRow>, String>
{
    repo.list_conversations(limit)
        .map_err(|e| e.to_string())
        .map(|cs| cs.into_iter().map(|c| ConvRow {
            id: c.id,
            title: c.title,
            updated_at: c.updated_at,
        }).collect())
}
```

- [ ] **Step 5: Replace command stubs with real bodies**

In `crates/coati-desktop/src/commands.rs`:

```rust
#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let endpoint = state.config.llm.endpoint.clone();
    let models = crate::ollama::list_installed(&endpoint).await.map_err(|e| e.to_string())?;
    Ok(models.into_iter().map(|(name, size)| ModelInfo { name, size }).collect())
}

#[tauri::command]
pub async fn list_conversations(_state: State<'_, AppState>) -> Result<Vec<ConvRow>, String> {
    let repo = coati_core::history::HistoryRepo::open_default().map_err(|e| e.to_string())?;
    list_conversations_from(&repo, 50).await
}

#[tauri::command]
pub async fn load_conversation(_state: State<'_, AppState>, id: String) -> Result<Vec<MsgRow>, String> {
    let repo = coati_core::history::HistoryRepo::open_default().map_err(|e| e.to_string())?;
    let ms = repo.messages(&id).map_err(|e| e.to_string())?;
    Ok(ms.into_iter().map(|m| MsgRow {
        role: m.role,
        content: m.content,
        created_at: m.created_at,
    }).collect())
}

#[tauri::command]
pub async fn create_conversation(state: State<'_, AppState>, title: String) -> Result<String, String> {
    let repo = coati_core::history::HistoryRepo::open_default().map_err(|e| e.to_string())?;
    let model = state.config.llm.model.clone();
    let conv = repo.create_conversation(&title, &model).map_err(|e| e.to_string())?;
    Ok(conv.id)
}
```

- [ ] **Step 6: Run `cargo test -p coati-desktop` → all pass**

- [ ] **Step 7: Commit**

```bash
git add crates/coati-desktop/
git commit -m "feat(desktop): implement list_models, list/load/create conversation

list_models queries ollama /api/tags; history commands use the shared
HistoryRepo from coati-core. Pure helper list_conversations_from is
extracted for unit testing without Tauri state."
```

---

## Task 9: Tauri command — `send_stream` bridges frontend to daemon

**Files:**
- Create: `crates/coati-desktop/src/stream.rs`
- Modify: `crates/coati-desktop/src/commands.rs`
- Modify: `crates/coati-desktop/src/main.rs`

- [ ] **Step 1: Create `crates/coati-desktop/src/stream.rs`**

```rust
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

use coati_core::ipc::{Request, Response};
use tauri::{AppHandle, Emitter};

pub async fn send_and_stream(
    socket: &Path,
    app: AppHandle,
    question: String,
    conversation_id: Option<String>,
) -> anyhow::Result<()> {
    let s = UnixStream::connect(socket)?;
    let mut writer = s.try_clone()?;
    let req = Request::AskStream { question, conversation_id };
    writeln!(writer, "{}", serde_json::to_string(&req)?)?;

    let reader = BufReader::new(s);
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() { continue; }
        let resp: Response = serde_json::from_str(&line)?;
        match resp {
            Response::Chunk { delta } => {
                let _ = app.emit("coati://chunk", delta);
            }
            Response::StreamEnd { full_content } => {
                let _ = app.emit("coati://end", full_content);
                break;
            }
            Response::Error { message } => {
                let _ = app.emit("coati://error", message);
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chunk_frame() {
        let s = r#"{"type":"chunk","delta":"hi"}"#;
        let r: Response = serde_json::from_str(s).unwrap();
        match r {
            Response::Chunk { delta } => assert_eq!(delta, "hi"),
            _ => panic!(),
        }
    }
}
```

We rely on `coati-cli tests/serve_stream.rs` (Task 4) for the full round-trip contract; here we just lock the client-side parse.

- [ ] **Step 2: Declare the module in `main.rs`**

Add:

```rust
mod stream;
```

- [ ] **Step 3: Replace the `send_stream` stub in `commands.rs`**

Add `use tauri::AppHandle;` at the top of `commands.rs`. Then:

```rust
#[tauri::command]
pub async fn send_stream(
    state: State<'_, AppState>,
    app: AppHandle,
    question: String,
    conversation_id: Option<String>,
) -> Result<(), String> {
    let sock = state.socket_path.clone();
    crate::stream::send_and_stream(&sock, app, question, conversation_id)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Run `cargo test -p coati-desktop` → passes**

- [ ] **Step 5: Commit**

```bash
git add crates/coati-desktop/
git commit -m "feat(desktop): send_stream bridges frontend events to daemon socket

Opens a Unix socket to the daemon, writes an AskStream request, then
emits three event types the frontend subscribes to: coati://chunk,
coati://end, coati://error."
```

---

## Task 10: Tauri command — `run_proposal` with confirm-before-sudo

**Files:**
- Create: `crates/coati-desktop/src/proposal.rs`
- Modify: `crates/coati-desktop/src/commands.rs`
- Modify: `crates/coati-desktop/src/main.rs`

- [ ] **Step 1: Create `crates/coati-desktop/src/proposal.rs`**

```rust
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub fn needs_sudo(cmd: &str) -> bool {
    let t = cmd.trim_start();
    t == "sudo" || t.starts_with("sudo ")
}

#[derive(serde::Serialize)]
pub struct RunResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub async fn run_confirmed(cmd: &str) -> anyhow::Result<RunResult> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut o) = child.stdout.take() { o.read_to_string(&mut stdout).await?; }
    if let Some(mut e) = child.stderr.take() { e.read_to_string(&mut stderr).await?; }
    let status = child.wait().await?;
    Ok(RunResult { stdout, stderr, exit_code: status.code().unwrap_or(-1) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sudo_detection() {
        assert!(needs_sudo("sudo systemctl restart nginx"));
        assert!(needs_sudo("sudo"));
        assert!(!needs_sudo("ls -la"));
        assert!(!needs_sudo("sudoify"));
    }

    #[tokio::test]
    async fn runs_a_safe_command() {
        let r = run_confirmed("echo hello").await.unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("hello"));
    }
}
```

**Design note:** we deliberately use `sh -c` here because a real proposal likely contains pipes, redirections, or env vars. The shell plugin also uses `eval`. The confirm-before-sudo guarantee lives at the UI boundary (Task 12): the frontend must never call `run_proposal` with `confirmed=true` unless the user clicked an explicit Yes button for a sudo command.

- [ ] **Step 2: Declare the module in `main.rs`**

Add:

```rust
mod proposal;
```

- [ ] **Step 3: Replace the `run_proposal` stub in `commands.rs`**

```rust
#[derive(Serialize)]
pub struct RunResult { pub stdout: String, pub stderr: String, pub exit_code: i32 }

#[tauri::command]
pub async fn run_proposal(
    _state: State<'_, AppState>,
    command: String,
    confirmed: bool,
) -> Result<RunResult, String> {
    if crate::proposal::needs_sudo(&command) && !confirmed {
        return Err("sudo command not confirmed".into());
    }
    let r = crate::proposal::run_confirmed(&command).await.map_err(|e| e.to_string())?;
    Ok(RunResult { stdout: r.stdout, stderr: r.stderr, exit_code: r.exit_code })
}
```

- [ ] **Step 4: Run `cargo test -p coati-desktop proposal::tests` → passes**

- [ ] **Step 5: Commit**

```bash
git add crates/coati-desktop/
git commit -m "feat(desktop): run_proposal rejects unconfirmed sudo commands

Backend refuses to run a proposal whose first token is 'sudo' unless
the frontend passes confirmed=true. The frontend (Task 12) only sets
that flag after an explicit Yes click."
```

---

## Task 11: Frontend scaffold — HTML/CSS/JS + bundled IBM Plex

**Files:**
- Modify: `crates/coati-desktop/dist/index.html`
- Create: `crates/coati-desktop/dist/app.css`
- Create: `crates/coati-desktop/dist/app.js`
- Add: `crates/coati-desktop/dist/fonts/IBMPlexSerif-Italic.woff2`
- Add: `crates/coati-desktop/dist/fonts/IBMPlexMono-Regular.woff2`
- Add: `crates/coati-desktop/dist/fonts/IBMPlexSans-Regular.woff2`

- [ ] **Step 1: Download the three fonts locally (CSP blocks remote origins)**

```bash
mkdir -p /home/marche/coati/crates/coati-desktop/dist/fonts
cd /home/marche/coati/crates/coati-desktop/dist/fonts
curl -fsSL -o IBMPlexSerif-Italic.woff2 https://github.com/IBM/plex/raw/master/packages/plex-serif/fonts/complete/woff2/IBMPlexSerif-Italic.woff2
curl -fsSL -o IBMPlexMono-Regular.woff2 https://github.com/IBM/plex/raw/master/packages/plex-mono/fonts/complete/woff2/IBMPlexMono-Regular.woff2
curl -fsSL -o IBMPlexSans-Regular.woff2 https://github.com/IBM/plex/raw/master/packages/plex-sans/fonts/complete/woff2/IBMPlexSans-Regular.woff2
```

Commit these binary files (each ~80KB).

- [ ] **Step 2: Write `dist/app.css`**

```css
@font-face {
  font-family: 'Plex Serif';
  src: url('fonts/IBMPlexSerif-Italic.woff2') format('woff2');
  font-weight: 400;
  font-style: italic;
}
@font-face {
  font-family: 'Plex Mono';
  src: url('fonts/IBMPlexMono-Regular.woff2') format('woff2');
  font-weight: 400;
  font-style: normal;
}
@font-face {
  font-family: 'Plex Sans';
  src: url('fonts/IBMPlexSans-Regular.woff2') format('woff2');
  font-weight: 400;
  font-style: normal;
}

:root {
  --terracotta: #E67347;
  --forest: #6AA07F;
  --ink: #1A130E;
  --surface: #2A1F16;
  --cream: #F5ECD8;
  --dim: #8C7F6F;
}

* { box-sizing: border-box; margin: 0; padding: 0; }
html, body { height: 100%; }
body {
  background: var(--ink);
  color: var(--cream);
  font-family: 'Plex Sans', system-ui, sans-serif;
  font-size: 14px;
  display: flex;
  flex-direction: column;
}

header {
  padding: 12px 16px;
  border-bottom: 1px solid var(--surface);
  display: flex;
  align-items: center;
  gap: 12px;
}
.wordmark { font-family: 'Plex Serif'; font-size: 20px; color: var(--terracotta); }
.model-select { margin-left: auto; font-family: 'Plex Mono'; background: var(--surface); color: var(--cream); border: 1px solid var(--dim); padding: 4px 8px; }

#messages {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.msg { max-width: 85%; padding: 8px 12px; border-radius: 6px; white-space: pre-wrap; }
.msg.user { align-self: flex-end; background: var(--terracotta); color: var(--ink); }
.msg.assistant { align-self: flex-start; background: var(--surface); }
.msg.assistant pre { font-family: 'Plex Mono'; background: var(--ink); padding: 6px; border-radius: 4px; margin: 6px 0; overflow-x: auto; }

#input-row {
  display: flex;
  padding: 12px;
  border-top: 1px solid var(--surface);
  gap: 8px;
}
#input {
  flex: 1;
  padding: 8px 12px;
  background: var(--surface);
  color: var(--cream);
  border: 1px solid var(--dim);
  border-radius: 4px;
  font-family: 'Plex Sans';
  font-size: 14px;
  resize: none;
}
#send {
  padding: 8px 16px;
  background: var(--terracotta);
  color: var(--ink);
  border: none;
  border-radius: 4px;
  font-family: 'Plex Mono';
  cursor: pointer;
}

.proposal {
  background: var(--surface);
  border: 1px solid var(--forest);
  padding: 12px;
  border-radius: 4px;
  margin: 8px 0;
}
.proposal code { font-family: 'Plex Mono'; color: var(--forest); }
.proposal.sudo { border-color: var(--terracotta); }
.proposal-actions { margin-top: 8px; display: flex; gap: 8px; }
.proposal-actions button { padding: 4px 12px; font-family: 'Plex Mono'; cursor: pointer; }
.btn-no { background: var(--surface); color: var(--cream); border: 1px solid var(--dim); }
.btn-yes { background: var(--forest); color: var(--ink); border: none; }
```

- [ ] **Step 3: Write minimal `dist/app.js` (chat loop wiring arrives in Task 12)**

```js
"use strict";

const { invoke } = window.__TAURI__.core;

function clear(node) {
  while (node.firstChild) node.removeChild(node.firstChild);
}

async function boot() {
  const models = await invoke("list_models").catch(() => []);
  const sel = document.getElementById("model");
  clear(sel);
  for (const m of models) {
    const opt = document.createElement("option");
    opt.value = m.name;
    opt.textContent = m.name;
    sel.appendChild(opt);
  }
}

document.addEventListener("DOMContentLoaded", boot);
```

- [ ] **Step 4: Rewrite `dist/index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>Coati</title>
    <link rel="stylesheet" href="app.css" />
  </head>
  <body>
    <header>
      <span class="wordmark">Coati</span>
      <select id="model" class="model-select"></select>
    </header>
    <div id="messages"></div>
    <div id="input-row">
      <textarea id="input" rows="2" placeholder="Ask coati something..."></textarea>
      <button id="send">Send</button>
    </div>
    <script src="app.js"></script>
  </body>
</html>
```

- [ ] **Step 5: Verify CSP allows what we use**

Check `tauri.conf.json` CSP: `font-src 'self'` (allows `/fonts/...`), `script-src 'self'` (allows `app.js`), `style-src 'self' 'unsafe-inline'` (allows `<link>` + any inline if needed), `connect-src 'self' ipc: http://ipc.localhost` (allows `invoke` IPC). No CDN entries — local-first verified.

- [ ] **Step 6: Build and smoke-test (skip if headless)**

```bash
cargo build -p coati-desktop --release
/home/marche/coati/target/release/coati-desktop &
# Window should show "Coati" header, empty chat area, input box
```

On headless CI this step is skipped — the build completing is sufficient.

- [ ] **Step 7: Commit**

```bash
git add crates/coati-desktop/dist/
git commit -m "feat(desktop): frontend scaffold with bundled IBM Plex fonts

Three WOFF2 files (~240KB total) ship inside the bundle; no CDN
requests. Chat shell, message list, and input row use the Coati
palette from brand.html. Real streaming wiring in Task 12."
```

---

## Task 12: Frontend chat UI + streaming display

**Files:**
- Modify: `crates/coati-desktop/dist/app.js`

- [ ] **Step 1: Replace `dist/app.js` with the full chat loop**

```js
"use strict";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let conversationId = null;
let currentAssistantEl = null;
let currentAssistantText = "";

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function clear(node) {
  while (node.firstChild) node.removeChild(node.firstChild);
}

function addMessage(role, content) {
  const msgs = document.getElementById("messages");
  const m = el("div", "msg " + role, content);
  msgs.appendChild(m);
  msgs.scrollTop = msgs.scrollHeight;
  return m;
}

function looksLikeProposal(text) {
  const m = text.match(/^\s*PROPOSE:\s*(.+)$/m);
  return m ? m[1].trim() : null;
}

function renderProposal(full) {
  const cmd = looksLikeProposal(full);
  if (!cmd) return null;
  const isSudo = /^\s*sudo(\s|$)/.test(cmd);
  const box = el("div", "proposal" + (isSudo ? " sudo" : ""));
  box.appendChild(el("div", null, "Proposed command:"));
  const code = el("code");
  code.textContent = cmd;
  box.appendChild(code);
  const actions = el("div", "proposal-actions");
  const no = el("button", "btn-no", isSudo ? "No (default)" : "No");
  const yes = el("button", "btn-yes", isSudo ? "Yes, run as sudo" : "Yes, run");
  actions.appendChild(no);
  actions.appendChild(yes);
  box.appendChild(actions);
  no.onclick = () => { box.remove(); };
  yes.onclick = async () => {
    no.disabled = true; yes.disabled = true;
    try {
      const res = await invoke("run_proposal", { command: cmd, confirmed: true });
      const out = el("pre");
      out.textContent = `exit ${res.exit_code}\n${res.stdout}${res.stderr}`;
      box.appendChild(out);
    } catch (e) {
      const err = el("pre");
      err.textContent = String(e);
      box.appendChild(err);
    }
  };
  return box;
}

async function send() {
  const input = document.getElementById("input");
  const q = input.value.trim();
  if (!q) return;
  input.value = "";

  if (!conversationId) {
    conversationId = await invoke("create_conversation", { title: q.slice(0, 40) });
  }

  addMessage("user", q);
  currentAssistantEl = addMessage("assistant", "");
  currentAssistantText = "";

  try {
    await invoke("send_stream", { question: q, conversationId });
  } catch (e) {
    currentAssistantEl.textContent = "Error: " + String(e);
  }
}

async function boot() {
  const models = await invoke("list_models").catch(() => []);
  const sel = document.getElementById("model");
  clear(sel);
  for (const m of models) {
    const opt = document.createElement("option");
    opt.value = m.name;
    opt.textContent = m.name;
    sel.appendChild(opt);
  }

  document.getElementById("send").onclick = send;
  document.getElementById("input").addEventListener("keydown", (ev) => {
    if (ev.key === "Enter" && !ev.shiftKey) {
      ev.preventDefault();
      send();
    }
  });

  await listen("coati://chunk", (ev) => {
    if (!currentAssistantEl) return;
    currentAssistantText += ev.payload;
    currentAssistantEl.textContent = currentAssistantText;
    const msgs = document.getElementById("messages");
    msgs.scrollTop = msgs.scrollHeight;
  });

  await listen("coati://end", (ev) => {
    if (!currentAssistantEl) return;
    const full = ev.payload || currentAssistantText;
    const proposal = renderProposal(full);
    if (proposal) {
      currentAssistantEl.after(proposal);
    }
    currentAssistantEl = null;
    currentAssistantText = "";
  });

  await listen("coati://error", (ev) => {
    if (currentAssistantEl) {
      currentAssistantEl.textContent = "Error: " + ev.payload;
      currentAssistantEl = null;
    }
  });
}

document.addEventListener("DOMContentLoaded", boot);
```

**Why the `PROPOSE:` convention:** the simplest way to trigger the confirm UX is a magic marker the LLM is prompted to emit. The frontend detects it, shows the command with default-No buttons, and calls `run_proposal` only after explicit Yes. For Phase 3 this lives entirely in the frontend; Phase 5 can formalize it via a tool-call path through `Request::Propose` when the desktop grows a dedicated propose affordance.

- [ ] **Step 2: Manual smoke (skip on headless CI)**

```bash
# Terminal 1
cargo run -p coati-cli -- serve
# Terminal 2
cargo run -p coati-desktop
# Type "hello"; response streams in. Type "how do I restart nginx" and
# verify the LLM emits `PROPOSE: sudo systemctl restart nginx` and the
# No/Yes buttons appear. Yes runs the command; the result appears inline.
```

- [ ] **Step 3: Commit**

```bash
git add crates/coati-desktop/dist/app.js
git commit -m "feat(desktop): chat loop with streaming + proposal confirmation

Subscribes to coati://chunk / coati://end / coati://error events from
send_stream. Detects PROPOSE: markers in the final content, renders
No/Yes buttons, and routes a Yes click through run_proposal with
confirmed=true. Default focus is No for sudo commands."
```

---

## Task 13: Frontend model selector + conversation sidebar

**Files:**
- Modify: `crates/coati-desktop/dist/index.html`
- Modify: `crates/coati-desktop/dist/app.css`
- Modify: `crates/coati-desktop/dist/app.js`

- [ ] **Step 1: Extend `index.html` with a sidebar**

Replace `<body>...</body>`:

```html
<body>
  <div id="layout">
    <aside id="sidebar">
      <div class="sidebar-head">
        <span class="wordmark">Coati</span>
      </div>
      <button id="new-conv">+ New</button>
      <ul id="conv-list"></ul>
    </aside>
    <main>
      <header>
        <select id="model" class="model-select"></select>
      </header>
      <div id="messages"></div>
      <div id="input-row">
        <textarea id="input" rows="2" placeholder="Ask coati something..."></textarea>
        <button id="send">Send</button>
      </div>
    </main>
  </div>
  <script src="app.js"></script>
</body>
```

- [ ] **Step 2: Append layout + sidebar CSS to `app.css`**

```css
#layout { display: flex; height: 100%; }
#sidebar {
  width: 160px;
  border-right: 1px solid var(--surface);
  display: flex;
  flex-direction: column;
  padding: 8px;
  gap: 8px;
}
.sidebar-head { padding: 6px 4px; }
#new-conv { background: var(--forest); color: var(--ink); border: none; padding: 6px; font-family: 'Plex Mono'; cursor: pointer; border-radius: 4px; }
#conv-list { list-style: none; overflow-y: auto; flex: 1; }
#conv-list li { padding: 6px; cursor: pointer; border-radius: 4px; font-size: 12px; color: var(--dim); }
#conv-list li:hover { background: var(--surface); color: var(--cream); }
#conv-list li.active { background: var(--terracotta); color: var(--ink); }
main { flex: 1; display: flex; flex-direction: column; }
```

- [ ] **Step 3: Append conversation list logic to `app.js`**

Add these functions before the `document.addEventListener("DOMContentLoaded", boot);` line:

```js
async function refreshConversations() {
  const list = document.getElementById("conv-list");
  clear(list);
  const rows = await invoke("list_conversations");
  for (const c of rows) {
    const li = el("li", null, c.title);
    if (c.id === conversationId) li.classList.add("active");
    li.onclick = () => loadConversation(c.id);
    list.appendChild(li);
  }
}

async function loadConversation(id) {
  const msgs = document.getElementById("messages");
  clear(msgs);
  conversationId = id;
  const rows = await invoke("load_conversation", { id });
  for (const r of rows) {
    addMessage(r.role, r.content);
  }
  refreshConversations();
}

function newConversation() {
  conversationId = null;
  clear(document.getElementById("messages"));
  refreshConversations();
}
```

Extend `boot()` so these fire on startup — add these lines inside `boot()`:

```js
  document.getElementById("new-conv").onclick = newConversation;
  await refreshConversations();
```

And update `send()` so the sidebar refreshes after a new conversation is created:

```js
  if (!conversationId) {
    conversationId = await invoke("create_conversation", { title: q.slice(0, 40) });
    await refreshConversations();
  }
```

- [ ] **Step 4: Build `cargo build -p coati-desktop` → green**

- [ ] **Step 5: Commit**

```bash
git add crates/coati-desktop/dist/
git commit -m "feat(desktop): conversation sidebar + model selector

Sidebar lists up to 50 most-recently-updated conversations; clicking
one loads its messages; + New clears the active conversation so the
next send() creates a fresh one."
```

---

## Task 14: Tray icon + tray menu

**Files:**
- Create: `crates/coati-desktop/src/tray.rs`
- Modify: `crates/coati-desktop/src/main.rs`
- Replace: `crates/coati-desktop/icons/tray-16.png`, `tray-32.png`, `icon-128.png`, `icon-512.png` (real art)

- [ ] **Step 1: Produce real icons**

The `brand.html` preview already has a coati SVG. Export four PNGs. If `inkscape` is available:

```bash
inkscape brand.svg -w 16  -o crates/coati-desktop/icons/tray-16.png   # hand-simplified variant
inkscape brand.svg -w 32  -o crates/coati-desktop/icons/tray-32.png
inkscape brand.svg -w 128 -o crates/coati-desktop/icons/icon-128.png
inkscape brand.svg -w 512 -o crates/coati-desktop/icons/icon-512.png
```

If no design work can happen in this task: substitute a single-color 16px coati silhouette rendered from the SVG at 16px using any tool. The tray glyph must remain legible at that size — do not just downscale the full-detail brand logo.

- [ ] **Step 2: Create `crates/coati-desktop/src/tray.rs`**

```rust
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};

pub fn init<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Chat", true, None::<&str>)?;
    let listen = MenuItem::with_id(app, "listen", "Toggle Listening (Phase 4)", false, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &listen, &sep, &settings, &sep, &quit])?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id.as_ref() {
            "open" => toggle_main(app),
            "settings" => {
                let _ = app.emit("coati://open-settings", ());
                toggle_main(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = ev
            {
                toggle_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn toggle_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        match w.is_visible() {
            Ok(true) => { let _ = w.hide(); }
            _ => {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
    }
}
```

- [ ] **Step 3: Wire `tray::init` from `main.rs`**

Replace the current `.setup(|_app| { Ok(()) })` block with:

```rust
        .setup(|app| {
            tray::init(app.handle())?;
            Ok(())
        })
```

And add:

```rust
mod tray;
```

- [ ] **Step 4: `cargo build -p coati-desktop` → compiles; run the app and confirm the tray icon appears and left-click toggles the window.**

- [ ] **Step 5: Commit**

```bash
git add crates/coati-desktop/
git commit -m "feat(desktop): tray icon with Open Chat / Settings / Quit menu

Toggle Listening is disabled with a (Phase 4) suffix as a visible
placeholder for voice. Left-click on the tray toggles the main window;
the window starts hidden and only appears via tray or global hotkey."
```

---

## Task 15: Global hotkey + window toggle

**Files:**
- Create: `crates/coati-desktop/src/shortcut.rs`
- Modify: `crates/coati-desktop/src/main.rs`

- [ ] **Step 1: Create `crates/coati-desktop/src/shortcut.rs`**

```rust
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn register<R: Runtime>(app: &AppHandle<R>, accelerator: &str) -> tauri::Result<()> {
    let shortcut: Shortcut = accelerator
        .parse()
        .map_err(|e: tauri_plugin_global_shortcut::Error| tauri::Error::Anyhow(e.into()))?;
    let gs = app.global_shortcut();
    let app_clone = app.clone();
    gs.on_shortcut(shortcut, move |_app, _sc, event| {
        if event.state() == ShortcutState::Pressed {
            toggle_main(&app_clone);
        }
    })?;
    Ok(())
}

fn toggle_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        match w.is_visible() {
            Ok(true) => { let _ = w.hide(); }
            _ => { let _ = w.show(); let _ = w.set_focus(); }
        }
    }
}
```

- [ ] **Step 2: Wire from `main.rs`**

Inside `.setup(|app| { ... })` after `tray::init`:

```rust
            let state = app.state::<AppState>();
            let hotkey = state.hotkey.clone();
            drop(state);
            if let Err(e) = shortcut::register(app.handle(), &hotkey) {
                tracing::warn!("failed to register hotkey {hotkey}: {e}; falling back to Ctrl+Space");
                let _ = shortcut::register(app.handle(), "Ctrl+Space");
            }
```

And add:

```rust
mod shortcut;
```

- [ ] **Step 3: Build & smoke**

```bash
cargo run -p coati-desktop
# In any other app, press Ctrl+Space; Coati window should appear.
# Press again; should hide.
```

- [ ] **Step 4: Commit**

```bash
git add crates/coati-desktop/
git commit -m "feat(desktop): global hotkey toggles chat window

Reads hotkey from [desktop].hotkey in config.toml; falls back to
Ctrl+Space if the accelerator fails to parse. Pressing the hotkey
while the window is visible hides it; pressing while hidden shows
and focuses it."
```

---

## Task 16: Settings window

**Files:**
- Create: `crates/coati-desktop/dist/settings.html`
- Create: `crates/coati-desktop/dist/settings.js`
- Modify: `crates/coati-desktop/dist/app.css` (append settings styles)
- Modify: `crates/coati-desktop/src/commands.rs` (implement `set_settings`)
- Modify: `crates/coati-desktop/src/main.rs` (listen for open-settings event)
- Modify: `crates/coati-desktop/tauri.conf.json` (add second window)

- [ ] **Step 1: Implement `set_settings` properly in `commands.rs`**

Replace the stub:

```rust
#[tauri::command]
pub async fn set_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    let mut cfg = (*state.config).clone();
    cfg.desktop = Some(coati_core::config::DesktopConfig {
        hotkey: settings.hotkey,
        theme: settings.theme,
        window_width: settings.window_width,
        window_height: settings.window_height,
        history_enabled: cfg.desktop.as_ref().map(|d| d.history_enabled).unwrap_or(true),
    });
    cfg.save().map_err(|e| e.to_string())?;
    Ok(())
}
```

The running app keeps its old hotkey until restart. Surface a "Restart Coati to apply" note in the UI.

- [ ] **Step 2: Add a second window to `tauri.conf.json`**

Under `"windows"`, append an entry:

```json
{
  "title": "Coati Settings",
  "label": "settings",
  "url": "settings.html",
  "width": 420,
  "height": 320,
  "visible": false,
  "resizable": false
}
```

- [ ] **Step 3: Create `dist/settings.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>Coati Settings</title>
    <link rel="stylesheet" href="app.css" />
  </head>
  <body class="settings">
    <header><span class="wordmark">Settings</span></header>
    <form id="settings-form">
      <label>Hotkey <input type="text" id="hotkey" value="Ctrl+Space" /></label>
      <label>Theme <input type="text" id="theme" value="coati" /></label>
      <label>Window width <input type="number" id="window_width" min="320" max="1600" value="480" /></label>
      <label>Window height <input type="number" id="window_height" min="400" max="1600" value="640" /></label>
      <button type="submit">Save</button>
      <p class="hint">Restart Coati to apply hotkey changes.</p>
    </form>
    <script src="settings.js"></script>
  </body>
</html>
```

- [ ] **Step 4: Create `dist/settings.js`**

```js
"use strict";
const { invoke } = window.__TAURI__.core;

async function load() {
  const s = await invoke("get_settings");
  document.getElementById("hotkey").value = s.hotkey;
  document.getElementById("theme").value = s.theme;
  document.getElementById("window_width").value = s.window_width;
  document.getElementById("window_height").value = s.window_height;
}

document.getElementById("settings-form").onsubmit = async (ev) => {
  ev.preventDefault();
  const settings = {
    hotkey: document.getElementById("hotkey").value,
    theme: document.getElementById("theme").value,
    window_width: parseInt(document.getElementById("window_width").value, 10),
    window_height: parseInt(document.getElementById("window_height").value, 10),
  };
  await invoke("set_settings", { settings });
  alert("Saved. Restart Coati to apply hotkey changes.");
};

document.addEventListener("DOMContentLoaded", load);
```

- [ ] **Step 5: Append settings styles to `app.css`**

```css
body.settings { padding: 16px; display: block; }
body.settings form { display: flex; flex-direction: column; gap: 12px; }
body.settings label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--dim); }
body.settings input { padding: 6px 8px; background: var(--surface); color: var(--cream); border: 1px solid var(--dim); border-radius: 4px; font-family: 'Plex Mono'; }
body.settings button { padding: 8px; background: var(--forest); color: var(--ink); border: none; border-radius: 4px; cursor: pointer; }
body.settings .hint { color: var(--dim); font-size: 11px; }
```

- [ ] **Step 6: Listen for `coati://open-settings` in `main.rs`**

Inside the `.setup(|app| { ... })` block (after the shortcut registration), add:

```rust
            let app_for_listen = app.handle().clone();
            app.listen_any("coati://open-settings", move |_ev| {
                if let Some(w) = app_for_listen.get_webview_window("settings") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            });
```

- [ ] **Step 7: Build + smoke**

```bash
cargo run -p coati-desktop
# Right-click tray → Settings → window appears.
# Edit hotkey to Ctrl+Alt+Space, Save → confirm ~/.config/coati/config.toml now has the new value.
```

- [ ] **Step 8: Commit**

```bash
git add crates/coati-desktop/
git commit -m "feat(desktop): settings window for hotkey/theme/window size

Persists to the existing [desktop] section of ~/.config/coati/config.toml.
Hotkey changes require a restart — surfaced via hint text. Settings
window is launched from the tray Settings menu item."
```

---

## Task 17: README + `install.sh` opt-in block for desktop

**Files:**
- Modify: `/home/marche/coati/README.md`
- Modify: `/home/marche/coati/shell/install.sh`
- Create: `/home/marche/coati/scripts/install-desktop.sh`

- [ ] **Step 1: Append a Desktop section to `README.md`**

```markdown
## Desktop (optional)

Coati ships an optional Tauri tray app. The CLI and shell plugins work
without it — install only if you want a chat window.

**Prereqs (Ubuntu/Debian):**

```
sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

**Install:**

```
cargo build -p coati-desktop --release
./scripts/install-desktop.sh
```

**Run:**

```
coati-desktop &
# or: systemctl --user enable --now coati-desktop
```

**Features:**
- Tray icon with Open Chat / Settings / Quit
- Global hotkey (default Ctrl+Space) — configurable at `~/.config/coati/config.toml`
- Streaming chat via local Unix socket (same daemon the CLI uses)
- Conversation history at `~/.local/share/coati/history.db`
- Confirm-before-sudo for `PROPOSE:` commands from the model
```

- [ ] **Step 2: Create `scripts/install-desktop.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail

BIN_SRC="$(dirname "$0")/../target/release/coati-desktop"
BIN_DST="$HOME/.local/bin/coati-desktop"

if [ ! -x "$BIN_SRC" ]; then
  echo "error: $BIN_SRC not found. Run 'cargo build -p coati-desktop --release' first." >&2
  exit 1
fi

mkdir -p "$(dirname "$BIN_DST")"
install -m 0755 "$BIN_SRC" "$BIN_DST"

APPS_DIR="$HOME/.local/share/applications"
mkdir -p "$APPS_DIR"
cat > "$APPS_DIR/coati-desktop.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Coati
Comment=Your Linux copilot
Exec=$BIN_DST
Icon=coati
Categories=Utility;
Terminal=false
StartupNotify=true
EOF

echo "Installed coati-desktop to $BIN_DST"
echo "Launch with: coati-desktop"
```

- [ ] **Step 3: `chmod +x scripts/install-desktop.sh`**

- [ ] **Step 4: Append an opt-in block to `shell/install.sh`**

Before the final success message, append:

```bash
if command -v coati-desktop >/dev/null 2>&1; then
  echo "Desktop app detected at $(command -v coati-desktop)."
else
  echo ""
  echo "Optional: install the desktop app with:"
  echo "  cargo build -p coati-desktop --release && ./scripts/install-desktop.sh"
fi
```

- [ ] **Step 5: Commit**

```bash
git add README.md scripts/install-desktop.sh shell/install.sh
git commit -m "docs(desktop): install instructions + opt-in installer script

Desktop is explicitly optional — shell/install.sh only mentions it,
never builds it. Users who want the tray run install-desktop.sh after
cargo build."
```

---

## Task 18: CI job for desktop build

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Append a `desktop` job to `.github/workflows/ci.yml`**

```yaml
  desktop:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - name: Install system deps
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev \
            libsoup-3.0-dev \
            libgtk-3-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            libssl-dev \
            pkg-config
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: "1.82"
      - uses: Swatinem/rust-cache@v2
      - name: Build coati-desktop
        run: cargo build -p coati-desktop --release
      - name: Run coati-desktop tests
        run: cargo test -p coati-desktop
```

- [ ] **Step 2: Push a branch and open a PR**

```bash
git checkout -b phase-3-ci
git add .github/workflows/ci.yml
git commit -m "ci: add desktop build job

Ubuntu 24.04 runner installs webkit2gtk + gtk deps and builds
coati-desktop in release mode. Matches the Rust pin (1.82) used by
the check job."
git push -u origin phase-3-ci
gh pr create --title "Phase 3: Desktop tray + chat" --body "$(cat <<'EOF'
## Summary
- Tauri 2.x tray app with chat window (Tasks 7-16)
- Streaming IPC via Unix socket (Tasks 2-4)
- SQLite history (Task 6)
- Bundled IBM Plex fonts, strict CSP (Task 11)
- Confirm-before-sudo for PROPOSE: commands (Tasks 10, 12)

## Test plan
- [ ] `cargo test --workspace` green
- [ ] `cargo build -p coati-desktop --release` green on Ubuntu 24.04
- [ ] Launch coati-desktop; Ctrl+Space toggles window
- [ ] Type a message; streamed tokens appear
- [ ] Tray menu items work (Open Chat, Settings, Quit)
- [ ] Settings persist to ~/.config/coati/config.toml

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 3: Once all jobs green, merge the PR and tag**

```bash
gh pr merge --squash
git checkout main
git pull
git tag -a v0.0.3-phase3 -m "Phase 3: desktop tray + chat shipped"
git push --tags
```

- [ ] **Step 4: No extra commit — the merge commit is the closing artifact.**

---

## Self-Review

**Spec coverage (against the user's Phase 3 brief):**

| Requirement | Task(s) |
|---|---|
| Tauri 2.x scaffold (Rust + vanilla JS) | 1, 7, 11 |
| Tray icon + simplified 16px glyph | 14 |
| Global hotkey Ctrl+Space configurable | 5, 15, 16 |
| Chat window with streaming | 2, 3, 4, 9, 12 |
| SQLite conversation history | 6, 8 |
| Model selector querying /api/tags | 8, 13 |
| IPC via Unix socket (reuse) | 4, 9 |
| Local-first — CSP + bundled fonts | 1, 7, 11 |
| Confirm-before-sudo UX | 10, 12 |
| Plugin-first — desktop optional | 1, 17 |
| Streaming protocol: AskStream / Chunk / StreamEnd | 2 |
| History schema: conversations + messages | 6 |
| Tray menu: Open Chat / Toggle Listening (disabled) / Settings / Quit | 14 |
| Settings in `[desktop]` section of config.toml | 5, 16 |
| cargo test for backend; frontend tests deferred | 1-10, 18 |
| Similar TDD shape to Phase 2 | all tasks |

All spec requirements covered.

**Placeholder scan:** no TBDs, TODOs, or vague "add error handling" — every step has concrete code or explicit commands. The one design-fallback note in Task 14 ("If no design work can happen in this task") names a specific substitution behavior, not a placeholder.

**Type consistency check:**
- `Request::AskStream { question, conversation_id }` — Task 2 defines; Tasks 4, 9 use.
- `Response::Chunk { delta }` + `Response::StreamEnd { full_content }` — Task 2 defines; Tasks 3, 4, 9, 12 use.
- `DesktopConfig { hotkey, theme, window_width, window_height, history_enabled }` — Task 5 defines; Tasks 7, 16 use.
- `HistoryRepo::open_default/open/create_conversation/append_message/messages/list_conversations` — Task 6 defines; Tasks 4, 8 use.
- `Settings { hotkey, theme, window_width, window_height }` frontend type — Task 7 declares; Tasks 8, 16 use (no `history_enabled` on the frontend object by design; Task 16 preserves it server-side).
- Tauri command names (`list_models`, `list_conversations`, `load_conversation`, `create_conversation`, `send_stream`, `run_proposal`, `get_settings`, `set_settings`) — Task 7 declares all eight; every later task matches exactly.
- Event names (`coati://chunk`, `coati://end`, `coati://error`, `coati://open-settings`) — consistent across Tasks 9, 12, 14, 16.

All types consistent.
