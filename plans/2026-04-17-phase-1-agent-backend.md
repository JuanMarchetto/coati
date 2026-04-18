# Coati Phase 1: Agent Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Rust agent backend foundation — a CLI that takes a natural-language question, reasons with a local LLM via rig.rs + ollama, calls typed system tools when helpful, and returns a response. Also exposes a Unix socket daemon mode so later phases (shell plugin, Tauri app, voice daemon) can connect.

**Architecture:** Three-crate Cargo workspace. `coati-core` is the agent runtime and tool trait. `coati-tools` implements the 5 MVP tools. `coati-cli` is the binary entrypoint — both a one-shot `ask` subcommand and a `serve` daemon. All LLM interaction routes through a thin `LlmProvider` trait so ollama can be swapped for Anthropic/OpenAI later (behind an explicit user opt-in).

**Tech Stack:** Rust 1.82+, rig-core 0.7+, tokio, clap (CLI), serde + serde_json, toml (config), tracing (structured logs), thiserror (error types), tempfile (tests), assert_cmd (integration tests), wiremock (for LLM mocking), sysinfo + nvml-wrapper (hardware detection), inquire (TUI).

---

## Decisions locked in (2026-04-18)

- **Cross-platform abstraction from day one** (option B). Platform-specific code (system log access, service management, IPC transport) is gated behind traits in `coati-core`. Phase 1 ships Linux impls only; Mac/Windows impls are future phases. **Subagent instructions:**
  - When implementing **Task 7 (query_logs)**, first define a `SystemLogProvider` trait in `crates/coati-core/src/system.rs`. Then implement `LinuxJournalLogProvider` behind `#[cfg(target_os = "linux")]` that wraps `journalctl`. `QueryLogsTool` takes `Arc<dyn SystemLogProvider>` via dependency injection.
  - When implementing **Task 13 (serve)**, first define an `IpcTransport` trait in `crates/coati-cli/src/ipc.rs`. Then implement `UnixSocketTransport` behind `#[cfg(unix)]`. `cmd_serve::run` takes the transport as a parameter.
  - Any other shell-outs to Linux-specific binaries (e.g., `systemctl` in future tools) must go through a trait in `coati-core`, never direct.
- **Package split** locked in for Phase 5: `coati-cli` (base, headless, TUI), `coati-desktop` (+Tauri), `coati-voice` (+whisper). Affects Phase 5 planning, not Phase 1 source layout.
- **Headless-first support:** every subcommand must work over SSH with no `$DISPLAY`. TUI uses `inquire` for interactive prompts; non-interactive flags (`--model`, `--headless`, `--yes`) always available for scripts.

Additional Phase 1 tasks added below: Task 15 (coati-hw crate), Task 16 (coati model subcommands), Task 17 (coati hw subcommand), Task 18 (coati setup TUI). New Phase 1 total: ~20 days, ~4 weeks.

---

## File Structure

```
/home/marche/local-agent/
├── Cargo.toml                    # workspace root
├── Cargo.lock
├── README.md                     # short — detail lives in CLAUDE.md + ROADMAP.md
├── LICENSE                       # Apache-2.0
├── rust-toolchain.toml           # pin Rust version
├── .github/
│   └── workflows/
│       └── ci.yml                # cargo fmt, clippy, test
├── crates/
│   ├── coati-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # public API
│   │       ├── agent.rs          # reasoning loop
│   │       ├── llm.rs            # LlmProvider trait + ollama impl
│   │       ├── tool.rs           # Tool trait, ToolRegistry
│   │       ├── system.rs         # SystemLogProvider trait + Linux impl
│   │       ├── config.rs         # Config struct + load/save
│   │       ├── session.rs        # conversation persistence
│   │       └── error.rs          # CoatiError enum
│   ├── coati-tools/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── exec.rs           # exec tool
│   │       ├── read_file.rs      # read_file tool
│   │       ├── list_dir.rs       # list_dir tool
│   │       ├── query_logs.rs     # query_logs tool (consumes SystemLogProvider)
│   │       └── explain_error.rs  # explain_error tool
│   ├── coati-hw/                 # hardware detection + model recommendation
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── detect.rs         # RAM/CPU/GPU/disk detection via sysinfo + nvml
│   │       ├── recommend.rs      # hardware → model tier matrix
│   │       └── benchmark.rs      # tok/s microbench
│   └── coati-cli/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs           # CLI entrypoint
│           ├── cmd_ask.rs        # `coati ask` subcommand
│           ├── cmd_serve.rs      # `coati serve` daemon
│           ├── cmd_model.rs      # `coati model list/pull/set/recommend/benchmark`
│           ├── cmd_hw.rs         # `coati hw` subcommand
│           ├── cmd_setup.rs      # `coati setup` TUI installer flow
│           └── ipc.rs            # IpcTransport trait + Unix socket impl
└── plans/
    └── 2026-04-17-phase-1-agent-backend.md  # this file
```

**Why three crates:** `coati-core` is a library future plugins can depend on. `coati-tools` is separable so users can disable specific tools by build-flag or config. `coati-cli` is thin — it's just a shell over `coati-core`.

---

## Task 1: Initialize Rust workspace and repo

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `LICENSE`, `README.md`
- Create: `crates/coati-core/Cargo.toml`, `crates/coati-core/src/lib.rs`
- Create: `crates/coati-tools/Cargo.toml`, `crates/coati-tools/src/lib.rs`
- Create: `crates/coati-cli/Cargo.toml`, `crates/coati-cli/src/main.rs`

- [ ] **Step 1: Initialize git repo**

```bash
cd /home/marche/local-agent
git init
git branch -M main
```

- [ ] **Step 2: Create workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["crates/coati-core", "crates/coati-tools", "crates/coati-cli"]

[workspace.package]
version = "0.0.1"
edition = "2021"
rust-version = "1.82"
license = "Apache-2.0"
repository = "https://github.com/YOU/coati"
authors = ["Juan Patricio Marchetto"]

[workspace.dependencies]
tokio = { version = "1.40", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
clap = { version = "4.5", features = ["derive"] }
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1.0"
rig-core = "0.7"
```

- [ ] **Step 3: Pin Rust toolchain**

Create `rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.82"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 4: Create .gitignore and LICENSE**

`.gitignore`:
```
/target
Cargo.lock
*.swp
.DS_Store
.env
/dist
```

`LICENSE`: Apache-2.0 full text (download from choosealicense.com/licenses/apache-2.0/).

- [ ] **Step 5: Scaffold three member crates**

```bash
cargo new --lib crates/coati-core --name coati-core
cargo new --lib crates/coati-tools --name coati-tools
cargo new --bin crates/coati-cli --name coati-cli
```

Edit each `crates/*/Cargo.toml` to use workspace inheritance:

```toml
[package]
name = "coati-core"  # or coati-tools / coati-cli
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
# fill in per crate
```

- [ ] **Step 6: Verify workspace builds**

```bash
cargo build --workspace
```

Expected: three empty crates compile with warnings about unused code.

- [ ] **Step 7: Commit**

```bash
git add .
git commit -m "chore: initialize Cargo workspace with coati-core, coati-tools, coati-cli crates"
```

---

## Task 2: Set up CI

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Write the CI workflow**

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:

jobs:
  check:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
```

- [ ] **Step 2: Push to verify CI runs** (do this after Task 1 push to remote)

Expected: CI runs green on a workspace of empty crates.

- [ ] **Step 3: Commit**

```bash
git add .github/
git commit -m "chore: add GitHub Actions CI (fmt, clippy, test)"
```

---

## Task 3: Define the `Tool` trait (coati-core)

**Files:**
- Create: `crates/coati-core/src/tool.rs`
- Modify: `crates/coati-core/src/lib.rs`
- Test: `crates/coati-core/src/tool.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Write the failing test**

In `crates/coati-core/src/tool.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;

    struct EchoTool;

    #[derive(serde::Deserialize, schemars::JsonSchema)]
    struct EchoInput {
        msg: String,
    }

    #[async_trait]
    impl Tool for EchoTool {
        type Input = EchoInput;
        const NAME: &'static str = "echo";
        const DESCRIPTION: &'static str = "Echoes its input back.";

        async fn call(&self, input: Self::Input) -> Result<serde_json::Value, ToolError> {
            Ok(json!({ "echoed": input.msg }))
        }
    }

    #[tokio::test]
    async fn tool_registry_dispatches_by_name() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);

        let result = registry
            .call("echo", json!({ "msg": "hi" }))
            .await
            .unwrap();

        assert_eq!(result, json!({ "echoed": "hi" }));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p coati-core tool::tests
```

Expected: compile error — `Tool` trait and `ToolRegistry` not defined.

- [ ] **Step 3: Implement the `Tool` trait and `ToolRegistry`**

```rust
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("invalid input: {0}")]
    InvalidInput(#[from] serde_json::Error),
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error("tool execution failed: {0}")]
    Execution(String),
}

#[async_trait]
pub trait Tool: Send + Sync + 'static {
    type Input: DeserializeOwned + JsonSchema + Send;
    const NAME: &'static str;
    const DESCRIPTION: &'static str;

    async fn call(&self, input: Self::Input) -> Result<serde_json::Value, ToolError>;
}

#[async_trait]
trait ErasedTool: Send + Sync {
    async fn call_json(&self, input: serde_json::Value) -> Result<serde_json::Value, ToolError>;
    fn schema(&self) -> serde_json::Value;
    fn description(&self) -> &'static str;
}

#[async_trait]
impl<T: Tool> ErasedTool for T {
    async fn call_json(&self, input: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        let typed: T::Input = serde_json::from_value(input)?;
        self.call(typed).await
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(T::Input)).unwrap()
    }
    fn description(&self) -> &'static str {
        T::DESCRIPTION
    }
}

pub struct ToolRegistry {
    tools: HashMap<&'static str, Box<dyn ErasedTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register<T: Tool>(&mut self, tool: T) {
        self.tools.insert(T::NAME, Box::new(tool));
    }

    pub async fn call(
        &self,
        name: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        let tool = self.tools.get(name).ok_or_else(|| ToolError::NotFound(name.into()))?;
        tool.call_json(input).await
    }

    pub fn descriptions(&self) -> Vec<(&'static str, &'static str, serde_json::Value)> {
        self.tools
            .iter()
            .map(|(name, t)| (*name, t.description(), t.schema()))
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self { Self::new() }
}
```

Add to `crates/coati-core/Cargo.toml`:

```toml
[dependencies]
async-trait = "0.1"
schemars = "0.8"
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio = { workspace = true, features = ["macros", "rt"] }
```

In `crates/coati-core/src/lib.rs`:

```rust
pub mod tool;
pub use tool::{Tool, ToolError, ToolRegistry};
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p coati-core tool::tests
```

Expected: 1 test passes.

- [ ] **Step 5: Commit**

```bash
git add crates/coati-core/
git commit -m "feat(core): add Tool trait and ToolRegistry with typed JSON dispatch"
```

---

## Task 4: Implement `exec` tool (coati-tools)

**Files:**
- Create: `crates/coati-tools/src/exec.rs`
- Modify: `crates/coati-tools/src/lib.rs`
- Test: inline

- [ ] **Step 1: Write failing tests**

In `crates/coati-tools/src/exec.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn exec_runs_simple_command() {
        let tool = ExecTool::default();
        let out = tool.call(serde_json::from_value(json!({
            "command": "echo",
            "args": ["hello"]
        })).unwrap()).await.unwrap();

        assert_eq!(out["exit_code"], 0);
        assert!(out["stdout"].as_str().unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn exec_captures_nonzero_exit() {
        let tool = ExecTool::default();
        let out = tool.call(serde_json::from_value(json!({
            "command": "false",
            "args": []
        })).unwrap()).await.unwrap();

        assert_eq!(out["exit_code"], 1);
    }

    #[tokio::test]
    async fn exec_does_not_interpret_shell() {
        let tool = ExecTool::default();
        // If shell interpretation happened, this would list files. It should not.
        let out = tool.call(serde_json::from_value(json!({
            "command": "echo",
            "args": ["$HOME"]
        })).unwrap()).await.unwrap();

        assert_eq!(out["stdout"].as_str().unwrap().trim(), "$HOME");
    }

    #[tokio::test]
    async fn exec_times_out() {
        let tool = ExecTool { timeout_secs: 1 };
        let result = tool.call(serde_json::from_value(json!({
            "command": "sleep",
            "args": ["5"]
        })).unwrap()).await;

        assert!(matches!(result, Err(coati_core::ToolError::Execution(_))));
    }
}
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p coati-tools exec::tests
```

Expected: compile error, `ExecTool` undefined.

- [ ] **Step 3: Implement the tool**

```rust
use async_trait::async_trait;
use coati_core::{Tool, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Deserialize, JsonSchema)]
pub struct ExecInput {
    /// The program to execute (not a shell command — no piping, redirection, or variable expansion).
    pub command: String,
    /// Arguments passed to the program, one per array element.
    #[serde(default)]
    pub args: Vec<String>,
}

pub struct ExecTool {
    pub timeout_secs: u64,
}

impl Default for ExecTool {
    fn default() -> Self { Self { timeout_secs: 30 } }
}

#[async_trait]
impl Tool for ExecTool {
    type Input = ExecInput;
    const NAME: &'static str = "exec";
    const DESCRIPTION: &'static str = "Execute a program (not a shell command). Arguments are passed literally — no shell interpretation, no piping, no redirection.";

    async fn call(&self, input: ExecInput) -> Result<serde_json::Value, ToolError> {
        let fut = Command::new(&input.command).args(&input.args).output();
        let out = timeout(Duration::from_secs(self.timeout_secs), fut)
            .await
            .map_err(|_| ToolError::Execution(format!("timed out after {}s", self.timeout_secs)))?
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        Ok(json!({
            "stdout": String::from_utf8_lossy(&out.stdout),
            "stderr": String::from_utf8_lossy(&out.stderr),
            "exit_code": out.status.code().unwrap_or(-1),
        }))
    }
}
```

Add to `crates/coati-tools/Cargo.toml`:

```toml
[dependencies]
coati-core = { path = "../coati-core" }
async-trait = "0.1"
schemars = "0.8"
serde.workspace = true
serde_json.workspace = true
tokio = { workspace = true, features = ["process", "time"] }
```

In `crates/coati-tools/src/lib.rs`:

```rust
pub mod exec;
pub use exec::ExecTool;
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p coati-tools exec::tests
```

Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/coati-tools/
git commit -m "feat(tools): implement exec tool with no-shell safety and timeout"
```

---

## Task 5: Implement `read_file` tool

**Files:**
- Create: `crates/coati-tools/src/read_file.rs`
- Modify: `crates/coati-tools/src/lib.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn reads_utf8_file() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "hello world").unwrap();
        let path = f.path().to_str().unwrap().to_owned();

        let tool = ReadFileTool::default();
        let out = tool.call(serde_json::from_value(json!({
            "path": path
        })).unwrap()).await.unwrap();

        assert_eq!(out["content"].as_str().unwrap().trim(), "hello world");
        assert_eq!(out["truncated"], false);
    }

    #[tokio::test]
    async fn truncates_large_files() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&vec![b'a'; 10_000]).unwrap();
        let path = f.path().to_str().unwrap().to_owned();

        let tool = ReadFileTool::default();
        let out = tool.call(serde_json::from_value(json!({
            "path": path,
            "max_bytes": 100
        })).unwrap()).await.unwrap();

        assert_eq!(out["content"].as_str().unwrap().len(), 100);
        assert_eq!(out["truncated"], true);
    }

    #[tokio::test]
    async fn rejects_missing_file() {
        let tool = ReadFileTool::default();
        let result = tool.call(serde_json::from_value(json!({
            "path": "/nonexistent/file/path"
        })).unwrap()).await;

        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Verify failure**

```bash
cargo test -p coati-tools read_file::tests
```

Expected: compile error.

- [ ] **Step 3: Implement**

```rust
use async_trait::async_trait;
use coati_core::{Tool, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncReadExt;

#[derive(Deserialize, JsonSchema)]
pub struct ReadFileInput {
    /// Absolute or relative path to the file.
    pub path: PathBuf,
    /// Maximum bytes to read. Defaults to 64 KiB.
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,
}

fn default_max_bytes() -> usize { 64 * 1024 }

pub struct ReadFileTool;

impl Default for ReadFileTool {
    fn default() -> Self { Self }
}

#[async_trait]
impl Tool for ReadFileTool {
    type Input = ReadFileInput;
    const NAME: &'static str = "read_file";
    const DESCRIPTION: &'static str = "Read the contents of a file up to max_bytes bytes. Use for logs, configs, source files.";

    async fn call(&self, input: ReadFileInput) -> Result<serde_json::Value, ToolError> {
        let mut file = fs::File::open(&input.path)
            .await
            .map_err(|e| ToolError::Execution(format!("open {}: {e}", input.path.display())))?;

        let mut buf = vec![0u8; input.max_bytes];
        let n = file.read(&mut buf).await
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        let truncated = {
            let mut extra = [0u8; 1];
            file.read(&mut extra).await.map(|r| r > 0).unwrap_or(false)
        };

        buf.truncate(n);
        let content = String::from_utf8_lossy(&buf).into_owned();

        Ok(json!({
            "content": content,
            "bytes_read": n,
            "truncated": truncated,
        }))
    }
}
```

Add to `crates/coati-tools/Cargo.toml` dev-dependencies: `tempfile = "3"`.

Update `crates/coati-tools/src/lib.rs`:

```rust
pub mod read_file;
pub use read_file::ReadFileTool;
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p coati-tools read_file::tests
```

Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/coati-tools/
git commit -m "feat(tools): implement read_file tool with truncation"
```

---

## Task 6: Implement `list_dir` tool

**Files:**
- Create: `crates/coati-tools/src/list_dir.rs`
- Modify: `crates/coati-tools/src/lib.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;
    use std::fs;

    #[tokio::test]
    async fn lists_flat_directory() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "").unwrap();
        fs::write(dir.path().join("b.txt"), "").unwrap();

        let tool = ListDirTool;
        let out = tool.call(serde_json::from_value(json!({
            "path": dir.path().to_str().unwrap()
        })).unwrap()).await.unwrap();

        let entries = out["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn distinguishes_files_and_directories() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let tool = ListDirTool;
        let out = tool.call(serde_json::from_value(json!({
            "path": dir.path().to_str().unwrap()
        })).unwrap()).await.unwrap();

        let types: Vec<&str> = out["entries"]
            .as_array().unwrap()
            .iter()
            .map(|e| e["kind"].as_str().unwrap())
            .collect();
        assert!(types.contains(&"file"));
        assert!(types.contains(&"directory"));
    }
}
```

- [ ] **Step 2: Implement**

```rust
use async_trait::async_trait;
use coati_core::{Tool, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use tokio::fs;

#[derive(Deserialize, JsonSchema)]
pub struct ListDirInput {
    pub path: PathBuf,
}

pub struct ListDirTool;

#[async_trait]
impl Tool for ListDirTool {
    type Input = ListDirInput;
    const NAME: &'static str = "list_dir";
    const DESCRIPTION: &'static str = "List files and subdirectories in a directory (non-recursive).";

    async fn call(&self, input: ListDirInput) -> Result<serde_json::Value, ToolError> {
        let mut rd = fs::read_dir(&input.path).await
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        let mut entries = Vec::new();
        while let Some(e) = rd.next_entry().await.map_err(|e| ToolError::Execution(e.to_string()))? {
            let md = e.metadata().await.map_err(|e| ToolError::Execution(e.to_string()))?;
            let kind = if md.is_dir() { "directory" }
                else if md.is_file() { "file" }
                else { "other" };
            entries.push(json!({
                "name": e.file_name().to_string_lossy(),
                "kind": kind,
                "size": md.len(),
            }));
        }

        Ok(json!({ "entries": entries }))
    }
}
```

- [ ] **Step 3: Run, verify, commit**

```bash
cargo test -p coati-tools list_dir::tests
git add crates/coati-tools/
git commit -m "feat(tools): implement list_dir tool"
```

---

## Task 7: Implement `query_logs` tool

**Files:**
- Create: `crates/coati-tools/src/query_logs.rs`
- Modify: `crates/coati-tools/src/lib.rs`

- [ ] **Step 1: Write test (integration-style — needs systemd)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    #[ignore] // requires systemd; run with `cargo test -- --ignored`
    async fn queries_systemd_unit_logs() {
        let tool = QueryLogsTool;
        let out = tool.call(serde_json::from_value(json!({
            "unit": "systemd-logind.service",
            "lines": 5
        })).unwrap()).await.unwrap();

        assert!(out["lines"].as_array().is_some());
    }

    #[tokio::test]
    async fn rejects_shell_injection_in_unit_name() {
        let tool = QueryLogsTool;
        let result = tool.call(serde_json::from_value(json!({
            "unit": "foo; rm -rf /",
            "lines": 5
        })).unwrap()).await;

        // unit names should be validated — reject anything with shell metacharacters
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Implement**

```rust
use async_trait::async_trait;
use coati_core::{Tool, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use tokio::process::Command;

#[derive(Deserialize, JsonSchema)]
pub struct QueryLogsInput {
    /// The systemd unit name (e.g. "nginx.service"). Must match [a-zA-Z0-9@._-]+.
    pub unit: String,
    /// How many recent log lines to fetch. Defaults to 50, max 500.
    #[serde(default = "default_lines")]
    pub lines: u32,
}

fn default_lines() -> u32 { 50 }

fn is_valid_unit_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '-'))
}

pub struct QueryLogsTool;

#[async_trait]
impl Tool for QueryLogsTool {
    type Input = QueryLogsInput;
    const NAME: &'static str = "query_logs";
    const DESCRIPTION: &'static str = "Fetch recent journalctl log lines for a systemd unit. Use when diagnosing service failures.";

    async fn call(&self, input: QueryLogsInput) -> Result<serde_json::Value, ToolError> {
        if !is_valid_unit_name(&input.unit) {
            return Err(ToolError::Execution(format!("invalid unit name: {}", input.unit)));
        }
        let lines = input.lines.min(500);

        let out = Command::new("journalctl")
            .args(["-u", &input.unit, "-n", &lines.to_string(), "--no-pager", "--output=short"])
            .output()
            .await
            .map_err(|e| ToolError::Execution(format!("journalctl: {e}")))?;

        let body = String::from_utf8_lossy(&out.stdout);
        let lines: Vec<&str> = body.lines().collect();

        Ok(json!({
            "unit": input.unit,
            "lines": lines,
            "stderr": String::from_utf8_lossy(&out.stderr),
        }))
    }
}
```

- [ ] **Step 3: Run, verify, commit**

```bash
cargo test -p coati-tools query_logs::tests
# integration test:
cargo test -p coati-tools query_logs::tests -- --ignored
git add crates/coati-tools/
git commit -m "feat(tools): implement query_logs wrapping journalctl with unit-name validation"
```

---

## Task 8: Implement `explain_error` tool

**Files:**
- Create: `crates/coati-tools/src/explain_error.rs`
- Modify: `crates/coati-tools/src/lib.rs`

This tool is unique: it doesn't do a system action — it reformats LLM-suitable context. It's registered as a tool so the agent can call it to get focused analysis of command output.

- [ ] **Step 1: Write test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn packages_command_output_for_analysis() {
        let tool = ExplainErrorTool;
        let out = tool.call(serde_json::from_value(json!({
            "command": "nginx -t",
            "stdout": "",
            "stderr": "nginx: [emerg] unknown directive \"worker_connecions\"",
            "exit_code": 1
        })).unwrap()).await.unwrap();

        let s = out["analysis_prompt"].as_str().unwrap();
        assert!(s.contains("nginx -t"));
        assert!(s.contains("worker_connecions"));
        assert!(s.contains("exit_code"));
    }
}
```

- [ ] **Step 2: Implement**

```rust
use async_trait::async_trait;
use coati_core::{Tool, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, JsonSchema)]
pub struct ExplainErrorInput {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub struct ExplainErrorTool;

#[async_trait]
impl Tool for ExplainErrorTool {
    type Input = ExplainErrorInput;
    const NAME: &'static str = "explain_error";
    const DESCRIPTION: &'static str = "Package a failed command's output for diagnosis. Call with the command string and its stdout/stderr/exit_code. Returns a focused analysis prompt the agent should reason over.";

    async fn call(&self, input: ExplainErrorInput) -> Result<serde_json::Value, ToolError> {
        let prompt = format!(
            "Diagnose why this command failed.\n\
             command: {}\n\
             exit_code: {}\n\
             stdout:\n{}\n\
             stderr:\n{}\n\
             Identify the root cause and propose a concrete fix.",
            input.command, input.exit_code, input.stdout, input.stderr
        );
        Ok(json!({ "analysis_prompt": prompt }))
    }
}
```

- [ ] **Step 3: Test, commit**

```bash
cargo test -p coati-tools explain_error::tests
git add crates/coati-tools/
git commit -m "feat(tools): implement explain_error tool for packaging failed-command context"
```

---

## Task 9: LlmProvider trait + ollama client (coati-core)

**Files:**
- Create: `crates/coati-core/src/llm.rs`
- Modify: `crates/coati-core/src/lib.rs`

- [ ] **Step 1: Write test with wiremock**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};

    #[tokio::test]
    async fn ollama_complete_returns_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": { "role": "assistant", "content": "hi" },
                "done": true
            })))
            .mount(&server).await;

        let client = OllamaClient::new(server.uri(), "gemma3".into());
        let msg = ChatMessage { role: "user".into(), content: "hey".into() };
        let resp = client.complete(&[msg], &[]).await.unwrap();

        assert_eq!(resp.content, "hi");
    }
}
```

- [ ] **Step 2: Implement trait + client**

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug)]
pub struct LlmResponse {
    pub content: String,
    pub tool_calls: Vec<LlmToolCall>,
}

#[derive(Debug, Deserialize)]
pub struct LlmToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[(&'static str, &'static str, serde_json::Value)],
    ) -> anyhow::Result<LlmResponse>;
}

pub struct OllamaClient {
    base_url: String,
    model: String,
    http: reqwest::Client,
}

impl OllamaClient {
    pub fn new(base_url: String, model: String) -> Self {
        Self { base_url, model, http: reqwest::Client::new() }
    }
}

#[async_trait]
impl LlmProvider for OllamaClient {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[(&'static str, &'static str, serde_json::Value)],
    ) -> anyhow::Result<LlmResponse> {
        let tools_json: Vec<_> = tools.iter().map(|(name, desc, schema)| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": name,
                    "description": desc,
                    "parameters": schema,
                }
            })
        }).collect();

        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "tools": tools_json,
            "stream": false,
        });

        let resp: serde_json::Value = self.http
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send().await?
            .error_for_status()?
            .json().await?;

        let msg = &resp["message"];
        let content = msg["content"].as_str().unwrap_or("").to_string();
        let tool_calls: Vec<LlmToolCall> = msg["tool_calls"].as_array()
            .cloned().unwrap_or_default()
            .into_iter()
            .filter_map(|tc| serde_json::from_value(tc["function"].clone()).ok())
            .collect();

        Ok(LlmResponse { content, tool_calls })
    }
}
```

Add to `crates/coati-core/Cargo.toml`:

```toml
reqwest = { version = "0.12", features = ["json"] }
anyhow.workspace = true

[dev-dependencies]
wiremock = "0.6"
```

- [ ] **Step 3: Run, commit**

```bash
cargo test -p coati-core llm::tests
git add crates/coati-core/
git commit -m "feat(core): add LlmProvider trait and OllamaClient"
```

---

## Task 10: Agent reasoning loop

**Files:**
- Create: `crates/coati-core/src/agent.rs`
- Modify: `crates/coati-core/src/lib.rs`

- [ ] **Step 1: Write test with a mock LlmProvider**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{LlmProvider, LlmResponse, LlmToolCall, ChatMessage};
    use crate::tool::{Tool, ToolRegistry, ToolError};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    struct ScriptedLlm { responses: Mutex<Vec<LlmResponse>> }

    #[async_trait]
    impl LlmProvider for ScriptedLlm {
        async fn complete(
            &self,
            _messages: &[ChatMessage],
            _tools: &[(&'static str, &'static str, serde_json::Value)],
        ) -> anyhow::Result<LlmResponse> {
            Ok(self.responses.lock().unwrap().remove(0))
        }
    }

    #[derive(serde::Deserialize, schemars::JsonSchema)]
    struct NopInput {}

    struct NopTool;

    #[async_trait]
    impl Tool for NopTool {
        type Input = NopInput;
        const NAME: &'static str = "nop";
        const DESCRIPTION: &'static str = "does nothing";
        async fn call(&self, _: NopInput) -> Result<serde_json::Value, ToolError> {
            Ok(serde_json::json!({ "result": "ok" }))
        }
    }

    #[tokio::test]
    async fn agent_handles_tool_call_then_final_response() {
        let llm = Arc::new(ScriptedLlm {
            responses: Mutex::new(vec![
                LlmResponse {
                    content: "".into(),
                    tool_calls: vec![LlmToolCall { name: "nop".into(), arguments: serde_json::json!({}) }],
                },
                LlmResponse {
                    content: "all done".into(),
                    tool_calls: vec![],
                },
            ])
        });

        let mut registry = ToolRegistry::new();
        registry.register(NopTool);

        let agent = Agent::new(llm, registry);
        let reply = agent.respond("do the thing").await.unwrap();
        assert_eq!(reply, "all done");
    }
}
```

- [ ] **Step 2: Implement**

```rust
use crate::llm::{ChatMessage, LlmProvider};
use crate::tool::ToolRegistry;
use std::sync::Arc;

pub struct Agent {
    llm: Arc<dyn LlmProvider>,
    tools: ToolRegistry,
    max_iterations: usize,
}

impl Agent {
    pub fn new(llm: Arc<dyn LlmProvider>, tools: ToolRegistry) -> Self {
        Self { llm, tools, max_iterations: 8 }
    }

    pub async fn respond(&self, user_input: &str) -> anyhow::Result<String> {
        let mut messages = vec![ChatMessage {
            role: "user".into(),
            content: user_input.into(),
        }];

        let descriptions = self.tools.descriptions();
        let tool_descs: Vec<(&'static str, &'static str, serde_json::Value)> =
            descriptions.iter().map(|(n, d, s)| (*n, *d, s.clone())).collect();

        for _ in 0..self.max_iterations {
            let resp = self.llm.complete(&messages, &tool_descs).await?;

            if resp.tool_calls.is_empty() {
                return Ok(resp.content);
            }

            messages.push(ChatMessage { role: "assistant".into(), content: resp.content.clone() });

            for call in resp.tool_calls {
                let result = self.tools.call(&call.name, call.arguments).await
                    .unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() }));
                messages.push(ChatMessage {
                    role: "tool".into(),
                    content: result.to_string(),
                });
            }
        }

        anyhow::bail!("agent exceeded {} iterations without a final answer", self.max_iterations);
    }
}
```

- [ ] **Step 3: Run, commit**

```bash
cargo test -p coati-core agent::tests
git add crates/coati-core/
git commit -m "feat(core): implement Agent reasoning loop with tool dispatch"
```

---

## Task 11: Config file loading

**Files:**
- Create: `crates/coati-core/src/config.rs`

- [ ] **Step 1: Write test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_config() {
        let toml = r#"
            [llm]
            provider = "ollama"
            endpoint = "http://localhost:11434"
            model = "gemma3"

            [tools]
            enabled = ["exec", "read_file"]
        "#;
        let c: Config = toml::from_str(toml).unwrap();
        assert_eq!(c.llm.model, "gemma3");
        assert!(c.tools.enabled.contains(&"exec".into()));
    }
}
```

- [ ] **Step 2: Implement**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub llm: LlmConfig,
    pub tools: ToolsConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ToolsConfig {
    #[serde(default)]
    pub enabled: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            llm: LlmConfig {
                provider: "ollama".into(),
                endpoint: "http://localhost:11434".into(),
                model: "gemma3".into(),
            },
            tools: ToolsConfig {
                enabled: vec!["exec", "read_file", "list_dir", "query_logs", "explain_error"]
                    .into_iter().map(String::from).collect(),
            },
        }
    }
}

impl Config {
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("coati/config.toml")
    }

    pub fn load_or_default() -> anyhow::Result<Self> {
        let path = Self::default_path();
        if path.exists() {
            let s = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&s)?)
        } else {
            Ok(Self::default())
        }
    }
}
```

Add `dirs = "5"` to coati-core's Cargo.toml.

- [ ] **Step 3: Test, commit**

```bash
cargo test -p coati-core config::tests
git add crates/coati-core/
git commit -m "feat(core): add Config with TOML load and XDG default path"
```

---

## Task 12: CLI `ask` subcommand

**Files:**
- Create: `crates/coati-cli/src/main.rs` (rewrite)
- Create: `crates/coati-cli/src/cmd_ask.rs`

- [ ] **Step 1: Write integration test**

In `crates/coati-cli/tests/ask.rs`:

```rust
use assert_cmd::Command;
use predicates::str::contains;

#[test]
#[ignore] // needs a running ollama
fn ask_returns_a_response() {
    Command::cargo_bin("coati").unwrap()
        .args(["ask", "say hello"])
        .assert()
        .success()
        .stdout(contains("hello").or(contains("Hello")));
}

#[test]
fn ask_without_args_shows_usage() {
    Command::cargo_bin("coati").unwrap()
        .arg("ask")
        .assert()
        .failure();
}
```

- [ ] **Step 2: Implement CLI**

`crates/coati-cli/src/main.rs`:

```rust
use clap::{Parser, Subcommand};

mod cmd_ask;
mod cmd_serve;

#[derive(Parser)]
#[command(name = "coati", version, about = "Your Linux copilot.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ask a one-shot question and print the answer.
    Ask {
        /// The question. If omitted, reads from stdin.
        question: Option<String>,
    },
    /// Run as a daemon exposing a Unix socket.
    Serve {
        #[arg(long, default_value = "~/.cache/coati/agent.sock")]
        socket: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("coati=info").init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Ask { question } => cmd_ask::run(question).await,
        Commands::Serve { socket } => cmd_serve::run(&socket).await,
    }
}
```

`crates/coati-cli/src/cmd_ask.rs`:

```rust
use coati_core::agent::Agent;
use coati_core::config::Config;
use coati_core::llm::OllamaClient;
use coati_core::tool::ToolRegistry;
use coati_tools::{ExecTool, ListDirTool, QueryLogsTool, ReadFileTool, explain_error::ExplainErrorTool};
use std::io::Read;
use std::sync::Arc;

pub async fn run(question: Option<String>) -> anyhow::Result<()> {
    let q = match question {
        Some(q) => q,
        None => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        }
    };

    let cfg = Config::load_or_default()?;

    let llm = Arc::new(OllamaClient::new(cfg.llm.endpoint.clone(), cfg.llm.model.clone()));

    let mut registry = ToolRegistry::new();
    let enabled: std::collections::HashSet<&str> = cfg.tools.enabled.iter().map(|s| s.as_str()).collect();
    if enabled.contains("exec")          { registry.register(ExecTool::default()); }
    if enabled.contains("read_file")     { registry.register(ReadFileTool); }
    if enabled.contains("list_dir")      { registry.register(ListDirTool); }
    if enabled.contains("query_logs")    { registry.register(QueryLogsTool); }
    if enabled.contains("explain_error") { registry.register(ExplainErrorTool); }

    let agent = Agent::new(llm, registry);
    let reply = agent.respond(&q).await?;
    println!("{}", reply);
    Ok(())
}
```

Stub `cmd_serve.rs` to compile:

```rust
pub async fn run(_socket: &str) -> anyhow::Result<()> {
    anyhow::bail!("serve subcommand is implemented in Task 13")
}
```

Add to `crates/coati-cli/Cargo.toml`:

```toml
coati-core = { path = "../coati-core" }
coati-tools = { path = "../coati-tools" }
clap.workspace = true
tokio.workspace = true
anyhow.workspace = true
tracing-subscriber.workspace = true

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
```

- [ ] **Step 3: Run tests, manual smoke test**

```bash
cargo test -p coati-cli
# manual:
ollama serve &
ollama pull gemma3
cargo run -p coati-cli -- ask "what is 2 + 2"
```

Expected: prints a reasonable answer.

- [ ] **Step 4: Commit**

```bash
git add crates/coati-cli/
git commit -m "feat(cli): implement coati ask subcommand with config-driven tool registration"
```

---

## Task 13: `coati serve` daemon with Unix socket IPC

**Files:**
- Create: `crates/coati-cli/src/cmd_serve.rs` (replace stub)
- Create: `crates/coati-cli/src/ipc.rs`

- [ ] **Step 1: Define IPC protocol + test**

`crates/coati-cli/src/ipc.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Ask { question: String },
    Ping,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Answer { content: String },
    Pong,
    Error { message: String },
}
```

Test (in `crates/coati-cli/tests/serve.rs`):

```rust
use assert_cmd::Command;
use std::thread;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[tokio::test]
async fn serve_responds_to_ping() {
    let sock = std::env::temp_dir().join(format!("coati-test-{}.sock", std::process::id()));
    let sock_str = sock.to_str().unwrap().to_owned();

    // spawn daemon in background
    let sock_clone = sock_str.clone();
    thread::spawn(move || {
        let _ = Command::cargo_bin("coati").unwrap()
            .args(["serve", "--socket", &sock_clone])
            .timeout(Duration::from_secs(3))
            .output();
    });

    // wait for socket to appear
    for _ in 0..20 {
        if sock.exists() { break; }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream.write_all(br#"{"type":"ping"}\n"#).await.unwrap();

    let mut buf = vec![0u8; 1024];
    let n = stream.read(&mut buf).await.unwrap();
    let response = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(response.contains("pong"));
}
```

- [ ] **Step 2: Implement daemon**

`crates/coati-cli/src/cmd_serve.rs`:

```rust
use crate::ipc::{Request, Response};
use coati_core::agent::Agent;
use coati_core::config::Config;
use coati_core::llm::OllamaClient;
use coati_core::tool::ToolRegistry;
use coati_tools::{ExecTool, ListDirTool, QueryLogsTool, ReadFileTool, explain_error::ExplainErrorTool};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

pub async fn run(socket_path: &str) -> anyhow::Result<()> {
    let expanded = shellexpand::tilde(socket_path).into_owned();
    let path = Path::new(&expanded);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(path);

    let cfg = Config::load_or_default()?;
    let llm = Arc::new(OllamaClient::new(cfg.llm.endpoint.clone(), cfg.llm.model.clone()));
    let mut registry = ToolRegistry::new();
    registry.register(ExecTool::default());
    registry.register(ReadFileTool);
    registry.register(ListDirTool);
    registry.register(QueryLogsTool);
    registry.register(ExplainErrorTool);
    let agent = Arc::new(Agent::new(llm, registry));

    let listener = UnixListener::bind(path)?;
    tracing::info!(socket = %path.display(), "coati daemon ready");

    loop {
        let (stream, _) = listener.accept().await?;
        let agent = agent.clone();
        tokio::spawn(async move { handle_conn(stream, agent).await });
    }
}

async fn handle_conn(stream: UnixStream, agent: Arc<Agent>) {
    let (rd, mut wr) = stream.into_split();
    let mut reader = BufReader::new(rd);
    let mut line = String::new();
    while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(Request::Ping) => Response::Pong,
            Ok(Request::Ask { question }) => match agent.respond(&question).await {
                Ok(content) => Response::Answer { content },
                Err(e) => Response::Error { message: e.to_string() },
            },
            Err(e) => Response::Error { message: format!("bad request: {e}") },
        };
        let s = serde_json::to_string(&resp).unwrap();
        let _ = wr.write_all(s.as_bytes()).await;
        let _ = wr.write_all(b"\n").await;
        line.clear();
    }
}
```

Add to coati-cli deps: `shellexpand = "3"`.

- [ ] **Step 3: Run test, commit**

```bash
cargo test -p coati-cli --test serve
git add crates/coati-cli/
git commit -m "feat(cli): implement coati serve daemon with Unix socket IPC"
```

---

## Task 14: Milestone 2 end-to-end smoke test

**Files:**
- Create: `tests/e2e/smoke.sh` (project root)

- [ ] **Step 1: Write smoke test script**

```bash
#!/usr/bin/env bash
set -euo pipefail

echo "=== Coati Phase 1 smoke test ==="

if ! command -v ollama >/dev/null; then
    echo "FAIL: ollama not installed. Install from https://ollama.com"
    exit 1
fi

if ! ollama list | grep -q gemma3; then
    echo "pulling gemma3..."
    ollama pull gemma3
fi

echo "--- smoke test 1: simple question ---"
OUT=$(echo "what is 2 + 2" | ./target/release/coati ask)
echo "response: $OUT"
[[ -n "$OUT" ]] || { echo "FAIL: empty response"; exit 1; }

echo "--- smoke test 2: tool-using question ---"
OUT=$(./target/release/coati ask "list files in /tmp")
echo "response: $OUT"

echo "--- smoke test 3: daemon ping ---"
./target/release/coati serve --socket /tmp/coati-smoke.sock &
DAEMON=$!
sleep 1
echo '{"type":"ping"}' | nc -U /tmp/coati-smoke.sock
kill $DAEMON

echo "=== all smoke tests passed ==="
```

- [ ] **Step 2: Run it**

```bash
cargo build --release
chmod +x tests/e2e/smoke.sh
./tests/e2e/smoke.sh
```

- [ ] **Step 3: Commit + tag**

```bash
git add tests/
git commit -m "chore: add phase 1 e2e smoke test"
git tag v0.0.1-phase1
```

---

## Task 15: `coati-hw` crate — hardware detection

**Files:**
- Create: `crates/coati-hw/Cargo.toml`, `crates/coati-hw/src/{lib.rs, detect.rs, recommend.rs, benchmark.rs}`
- Modify: workspace `Cargo.toml` (add `coati-hw` to members)

- [ ] **Step 1: Write failing tests**

In `crates/coati-hw/src/detect.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ram() {
        let info = detect();
        assert!(info.ram_total_bytes > 0);
        assert!(info.ram_available_bytes > 0);
        assert!(info.ram_available_bytes <= info.ram_total_bytes);
    }

    #[test]
    fn detects_cpu() {
        let info = detect();
        assert!(info.cpu_cores > 0);
        assert!(!info.cpu_model.is_empty());
    }
}
```

In `crates/coati-hw/src/recommend.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::HardwareInfo;

    fn mk(ram_gb: u64, vram_gb: Option<u64>) -> HardwareInfo {
        HardwareInfo {
            ram_total_bytes: ram_gb * 1024 * 1024 * 1024,
            ram_available_bytes: ram_gb * 1024 * 1024 * 1024 * 80 / 100,
            cpu_cores: 8,
            cpu_model: "test".into(),
            has_avx2: true,
            has_avx512: false,
            gpus: vram_gb.into_iter().map(|v| GpuInfo {
                vendor: "NVIDIA".into(),
                name: "test".into(),
                vram_bytes: v * 1024 * 1024 * 1024,
            }).collect(),
            disk_free_bytes: 100 * 1024 * 1024 * 1024,
        }
    }

    #[test]
    fn recommends_small_model_for_8gb_ram_no_gpu() {
        let recs = recommend(&mk(8, None));
        let top = &recs[0];
        assert!(top.model.contains("4b") || top.model.contains("3b"));
    }

    #[test]
    fn recommends_larger_model_when_gpu_available() {
        let recs = recommend(&mk(16, Some(8)));
        let top = &recs[0];
        assert!(top.model.contains("14b") || top.model.contains("9b"));
    }

    #[test]
    fn excludes_models_that_wont_fit() {
        let recs = recommend(&mk(8, None));
        assert!(recs.iter().all(|r| !r.model.contains("70b")));
    }
}
```

- [ ] **Step 2: Implement detection** (`detect.rs`)

```rust
use sysinfo::{System, Disks};

#[derive(Debug, Clone)]
pub struct HardwareInfo {
    pub ram_total_bytes: u64,
    pub ram_available_bytes: u64,
    pub cpu_cores: usize,
    pub cpu_model: String,
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub gpus: Vec<GpuInfo>,
    pub disk_free_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub vendor: String,
    pub name: String,
    pub vram_bytes: u64,
}

pub fn detect() -> HardwareInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_model = sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_default();
    let flags = read_cpu_flags();

    HardwareInfo {
        ram_total_bytes: sys.total_memory(),
        ram_available_bytes: sys.available_memory(),
        cpu_cores: sys.cpus().len(),
        cpu_model,
        has_avx2: flags.contains("avx2"),
        has_avx512: flags.contains("avx512f"),
        gpus: detect_gpus(),
        disk_free_bytes: Disks::new_with_refreshed_list()
            .iter()
            .find(|d| d.mount_point() == std::path::Path::new("/home") || d.mount_point() == std::path::Path::new("/"))
            .map(|d| d.available_space())
            .unwrap_or(0),
    }
}

#[cfg(target_os = "linux")]
fn read_cpu_flags() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("flags"))
        .map(String::from)
        .unwrap_or_default()
}

#[cfg(not(target_os = "linux"))]
fn read_cpu_flags() -> String { String::new() }

fn detect_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    // Try NVML first (Nvidia)
    if let Ok(nvml) = nvml_wrapper::Nvml::init() {
        if let Ok(count) = nvml.device_count() {
            for i in 0..count {
                if let Ok(dev) = nvml.device_by_index(i) {
                    let name = dev.name().unwrap_or_default();
                    let mem = dev.memory_info().map(|m| m.total).unwrap_or(0);
                    gpus.push(GpuInfo { vendor: "NVIDIA".into(), name, vram_bytes: mem });
                }
            }
        }
    }
    // AMD detection via subprocess fallback
    if gpus.is_empty() {
        if let Ok(out) = std::process::Command::new("rocm-smi")
            .args(["--showmeminfo", "vram", "--csv"])
            .output()
        {
            if out.status.success() {
                // parse CSV; left as simple fallback
                let s = String::from_utf8_lossy(&out.stdout);
                for line in s.lines().skip(1) {
                    let cols: Vec<&str> = line.split(',').collect();
                    if cols.len() >= 2 {
                        if let Ok(bytes) = cols[1].trim().parse::<u64>() {
                            gpus.push(GpuInfo { vendor: "AMD".into(), name: cols[0].into(), vram_bytes: bytes });
                        }
                    }
                }
            }
        }
    }
    gpus
}
```

- [ ] **Step 3: Implement recommendations** (`recommend.rs`)

```rust
use crate::detect::HardwareInfo;

#[derive(Debug, Clone)]
pub struct ModelRecommendation {
    pub model: String,
    pub estimated_tok_per_sec: f32,
    pub reason: String,
    pub fits: bool,
}

pub fn recommend(hw: &HardwareInfo) -> Vec<ModelRecommendation> {
    let ram_gb = hw.ram_total_bytes / (1024 * 1024 * 1024);
    let vram_gb = hw.gpus.iter().map(|g| g.vram_bytes).max().unwrap_or(0) / (1024 * 1024 * 1024);
    let has_gpu = vram_gb >= 4;

    let candidates = vec![
        ("gemma3:4b",        3,  None,     8.0,  15.0),
        ("gemma3:9b-q4",     6,  None,     6.0,  12.0),
        ("qwen2.5:7b",       5,  None,     7.0,  14.0),
        ("qwen2.5:14b-q5",   11, Some(8),  25.0, 40.0),
        ("qwen2.5:32b-q4",   22, Some(16), 12.0, 25.0),
        ("llama3.3:70b-q4",  45, Some(24), 8.0,  18.0),
    ];

    let mut out = Vec::new();
    for (model, ram_need_gb, vram_need_gb, cpu_tps, gpu_tps) in candidates {
        let fits_cpu = ram_gb >= ram_need_gb + 2;
        let fits_gpu = vram_need_gb.map(|v| vram_gb >= v).unwrap_or(false);
        let fits = fits_cpu || fits_gpu;
        let tps = if fits_gpu && has_gpu { gpu_tps } else { cpu_tps };
        let reason = if !fits {
            format!("needs {}GB RAM or {}GB VRAM — insufficient on this system", ram_need_gb + 2, vram_need_gb.unwrap_or(0))
        } else if fits_gpu {
            format!("fits in {}GB VRAM, ~{:.0} tok/s", vram_gb, tps)
        } else {
            format!("CPU only, ~{:.0} tok/s", tps)
        };
        out.push(ModelRecommendation { model: model.into(), estimated_tok_per_sec: tps, reason, fits });
    }
    out.sort_by(|a, b| b.fits.cmp(&a.fits).then(b.estimated_tok_per_sec.partial_cmp(&a.estimated_tok_per_sec).unwrap()));
    out
}
```

- [ ] **Step 4: Stub benchmark** (`benchmark.rs` — full impl in later phase)

```rust
use anyhow::Result;

pub struct BenchmarkResult {
    pub tok_per_sec: f32,
    pub latency_ms: u32,
}

pub async fn benchmark(_endpoint: &str, _model: &str) -> Result<BenchmarkResult> {
    // MVP: single 20-token prompt, measure tok/s
    anyhow::bail!("benchmark not yet implemented — Phase 1 task 15 step 4 stub")
}
```

- [ ] **Step 5: Wire up lib.rs**

```rust
pub mod detect;
pub mod recommend;
pub mod benchmark;

pub use detect::{HardwareInfo, GpuInfo, detect};
pub use recommend::{ModelRecommendation, recommend};
```

- [ ] **Step 6: Add deps to Cargo.toml**

```toml
[dependencies]
sysinfo = "0.31"
nvml-wrapper = "0.10"
anyhow.workspace = true
```

- [ ] **Step 7: Run tests, commit**

```bash
cargo test -p coati-hw
git add crates/coati-hw/ Cargo.toml
git commit -m "feat(hw): add hardware detection and model recommendation"
```

---

## Task 16: `coati model` subcommands

**Files:**
- Create: `crates/coati-cli/src/cmd_model.rs`
- Modify: `crates/coati-cli/src/main.rs`

- [ ] **Step 1: Add `Model` subcommand variant in `main.rs`**

```rust
#[derive(Subcommand)]
enum Commands {
    Ask { question: Option<String> },
    Serve { #[arg(long, default_value = "~/.cache/coati/agent.sock")] socket: String },
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },
}

#[derive(Subcommand)]
enum ModelAction {
    /// List models installed in ollama
    List,
    /// Pull a model via ollama
    Pull { name: String },
    /// Set the active model in config
    Set { name: String },
    /// Show hardware-based recommendations
    Recommend,
    /// Benchmark the currently-configured model
    Benchmark,
}
```

- [ ] **Step 2: Implement each action** (`cmd_model.rs`)

```rust
use coati_core::config::Config;
use coati_hw::{detect, recommend};

pub async fn list() -> anyhow::Result<()> {
    let cfg = Config::load_or_default()?;
    let resp: serde_json::Value = reqwest::Client::new()
        .get(format!("{}/api/tags", cfg.llm.endpoint))
        .send().await?.json().await?;
    for m in resp["models"].as_array().unwrap_or(&vec![]) {
        println!("{}", m["name"].as_str().unwrap_or("?"));
    }
    Ok(())
}

pub async fn pull(name: &str) -> anyhow::Result<()> {
    let cfg = Config::load_or_default()?;
    let status = std::process::Command::new("ollama")
        .args(["pull", name])
        .env("OLLAMA_HOST", &cfg.llm.endpoint)
        .status()?;
    if !status.success() { anyhow::bail!("ollama pull failed") }
    Ok(())
}

pub fn set(name: &str) -> anyhow::Result<()> {
    let mut cfg = Config::load_or_default()?;
    cfg.llm.model = name.into();
    cfg.save()?;
    println!("active model set to: {}", name);
    Ok(())
}

pub fn recommend_cmd() -> anyhow::Result<()> {
    let hw = detect();
    println!("Hardware detected:");
    println!("  RAM:  {} GB total, {} GB available",
        hw.ram_total_bytes / 1_073_741_824,
        hw.ram_available_bytes / 1_073_741_824);
    println!("  CPU:  {} ({} cores)", hw.cpu_model, hw.cpu_cores);
    for gpu in &hw.gpus {
        println!("  GPU:  {} ({} GB VRAM)", gpu.name, gpu.vram_bytes / 1_073_741_824);
    }
    if hw.gpus.is_empty() { println!("  GPU:  none detected"); }
    println!();
    println!("Recommended models:");
    for rec in recommend(&hw).iter().take(5) {
        let marker = if rec.fits { "  " } else { "✗ " };
        println!("{}{:24} — {}", marker, rec.model, rec.reason);
    }
    Ok(())
}

pub async fn benchmark() -> anyhow::Result<()> {
    let cfg = Config::load_or_default()?;
    let result = coati_hw::benchmark::benchmark(&cfg.llm.endpoint, &cfg.llm.model).await?;
    println!("{} — {:.1} tok/s, {}ms first-token", cfg.llm.model, result.tok_per_sec, result.latency_ms);
    Ok(())
}
```

Config needs a `save()` method (add to `coati-core/src/config.rs`):

```rust
impl Config {
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::default_path();
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}
```

- [ ] **Step 3: Dispatch in main.rs**

```rust
Commands::Model { action } => match action {
    ModelAction::List => cmd_model::list().await,
    ModelAction::Pull { name } => cmd_model::pull(&name).await,
    ModelAction::Set { name } => cmd_model::set(&name),
    ModelAction::Recommend => cmd_model::recommend_cmd(),
    ModelAction::Benchmark => cmd_model::benchmark().await,
},
```

Add `coati-hw = { path = "../coati-hw" }` to `coati-cli/Cargo.toml`.

- [ ] **Step 4: Manual smoke + commit**

```bash
cargo run -- model list
cargo run -- model recommend
git add .
git commit -m "feat(cli): add coati model subcommands (list/pull/set/recommend/benchmark)"
```

---

## Task 17: `coati hw` subcommand

**Files:**
- Create: `crates/coati-cli/src/cmd_hw.rs`
- Modify: `crates/coati-cli/src/main.rs`

- [ ] **Step 1: Add `Hw` variant**

```rust
enum Commands {
    // ...
    /// Print detected hardware and model recommendations
    Hw,
}
```

- [ ] **Step 2: Implement** (just a thin wrapper — reuse `cmd_model::recommend_cmd` logic or extract)

```rust
pub fn run() -> anyhow::Result<()> {
    crate::cmd_model::recommend_cmd()
}
```

- [ ] **Step 3: Dispatch, smoke test, commit**

```bash
cargo run -- hw
git add crates/coati-cli/
git commit -m "feat(cli): add coati hw subcommand"
```

---

## Task 18: `coati setup` — TUI installer

**Files:**
- Create: `crates/coati-cli/src/cmd_setup.rs`
- Modify: `crates/coati-cli/src/main.rs`

The goal: first-run experience. Detect hardware, show recommendations, let the user pick a model, pull it, write config. Works over SSH. Also runnable later (`coati setup --reconfigure`).

- [ ] **Step 1: Add `Setup` variant**

```rust
enum Commands {
    /// First-run TUI: pick a model and initialize config
    Setup {
        #[arg(long)] reconfigure: bool,
        #[arg(long)] yes: bool,
        #[arg(long)] model: Option<String>,
    },
}
```

- [ ] **Step 2: Implement TUI flow** (`cmd_setup.rs`)

```rust
use coati_core::config::Config;
use coati_hw::{detect, recommend};
use inquire::Select;

pub async fn run(reconfigure: bool, yes: bool, model_override: Option<String>) -> anyhow::Result<()> {
    let config_path = Config::default_path();
    if config_path.exists() && !reconfigure {
        println!("Config already exists at {}. Use --reconfigure to start over.", config_path.display());
        return Ok(());
    }

    println!("Welcome to coati. Detecting hardware...\n");
    let hw = detect();
    println!("  RAM:  {} GB, CPU: {} ({} cores)",
        hw.ram_total_bytes / 1_073_741_824, hw.cpu_model, hw.cpu_cores);
    for gpu in &hw.gpus {
        println!("  GPU:  {} ({} GB VRAM)", gpu.name, gpu.vram_bytes / 1_073_741_824);
    }
    println!();

    let recs = recommend(&hw);
    let viable: Vec<_> = recs.into_iter().filter(|r| r.fits).collect();
    if viable.is_empty() {
        anyhow::bail!("no viable local models for this hardware — consider remote inference (documented in README)");
    }

    let chosen = if let Some(name) = model_override {
        name
    } else if yes {
        viable[0].model.clone()
    } else {
        let options: Vec<String> = viable.iter().map(|r| format!("{:24} — {}", r.model, r.reason)).collect();
        let pick = Select::new("Choose a model:", options).prompt()?;
        pick.split_whitespace().next().unwrap().to_string()
    };

    println!("\nPulling {} via ollama (this may take a while)...", chosen);
    let status = std::process::Command::new("ollama").args(["pull", &chosen]).status()?;
    if !status.success() { anyhow::bail!("ollama pull failed") }

    let mut cfg = Config::default();
    cfg.llm.model = chosen.clone();
    cfg.save()?;
    println!("\n✓ config written to {}", config_path.display());
    println!("✓ model {} ready", chosen);
    println!("\nTry: coati ask \"what is my disk usage\"");
    Ok(())
}
```

- [ ] **Step 3: Add `inquire = \"0.7\"` to coati-cli/Cargo.toml, wire dispatch**

```rust
Commands::Setup { reconfigure, yes, model } => cmd_setup::run(reconfigure, yes, model).await,
```

- [ ] **Step 4: Manual test (interactive + non-interactive), commit**

```bash
# Interactive
cargo run -- setup
# Non-interactive (CI-safe)
cargo run -- setup --yes
# Override
cargo run -- setup --model gemma3:4b --yes

git add .
git commit -m "feat(cli): add coati setup TUI installer with hardware-aware model selection"
```

---

## Phase 1 Exit Checkpoint

Before moving to Phase 2, confirm:

- [ ] `cargo test --workspace` passes with zero warnings
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `./tests/e2e/smoke.sh` passes on a fresh clone (with ollama running)
- [ ] `coati ask "why did nginx fail?"` with a real nginx failure returns a useful response that calls `query_logs`
- [ ] `coati serve` starts a daemon and `echo '{"type":"ping"}' | nc -U <sock>` returns `{"type":"pong"}`
- [ ] Config at `~/.config/coati/config.toml` controls model and tool allowlist
- [ ] `coati hw` prints hardware info and recommendations; `coati model list` works
- [ ] `coati setup --yes` completes an end-to-end install on a fresh machine (with ollama running)
- [ ] Platform abstraction traits (`SystemLogProvider`, `IpcTransport`) are used — no `journalctl`/Unix-socket references outside `#[cfg(...)]`-gated impls
- [ ] CI runs green on main
- [ ] README.md has basic install-from-source instructions including `coati setup`
- [ ] Repo is public on GitHub (optional but recommended — builds in public is part of the portfolio story)

When all boxes are checked, tag `v0.0.1-phase1` and start Phase 2 (shell integration).
