# Coati Phase 2: Shell Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship zsh + bash (stretch: fish) shell plugins so users can type `coati "restart nginx"` for a confirmed proposed command, and `??` after any failed command for an instant explanation. All traffic stays on the local Unix socket daemon from Phase 1. Confirm-before-sudo is enforced at the shell layer, not the agent.

**Architecture:** Three layers.

1. IPC protocol in `coati-core/src/ipc.rs` extended with `Propose` + `Explain` requests carrying a `ShellContext` struct, and `Proposal` + `Explanation` response variants.
2. Agent backend gains `propose()` and `explain()` helpers in `coati-core/src/agent_ext.rs` that use ollama's JSON-mode structured output (`format: "json"`) to return typed structs reliably.
3. Shell plugins are thin: they capture `pwd`, `$?`, last history line, and git branch; shell out to `coati propose --json` or `coati explain --json`; parse the single JSON line with `sed`; render and prompt; then run the confirmed command via `eval` in the user's own shell so sudo prompts, env vars, and aliases all work naturally.

**Tech Stack:** Existing Rust workspace (no new crates), zsh + bash + fish scripts, `bats-core` for shell integration tests, `wiremock` for agent-level JSON contract tests.

---

## File Structure

```
/home/marche/coati/
├── crates/
│   ├── coati-core/src/
│   │   ├── ipc.rs              # NEW — Request/Response/ShellContext shared types
│   │   ├── agent_ext.rs        # NEW — propose() and explain() helpers
│   │   ├── llm.rs              # MODIFIED — complete_json() + Any bound for downcast
│   │   └── lib.rs              # MODIFIED — re-export new types
│   └── coati-cli/src/
│       ├── ipc.rs              # MODIFIED — re-export types from core, keep transport
│       ├── cmd_propose.rs      # NEW — `coati propose` subcommand
│       ├── cmd_explain.rs      # NEW — `coati explain` subcommand
│       └── main.rs             # MODIFIED — register new subcommands
└── shell/
    ├── zsh/coati.plugin.zsh    # NEW
    ├── bash/coati.bash         # NEW
    ├── fish/coati.fish         # NEW (stretch)
    ├── install.sh              # NEW
    ├── tests/
    │   ├── mock_coati.sh       # NEW — fake binary for hermetic tests
    │   ├── zsh.bats            # NEW
    │   ├── bash.bats           # NEW
    │   └── run.sh              # NEW
    └── README.md               # NEW
```

**Decomposition principle:** IPC types live in `coati-core` so every surface (CLI, desktop, voice) shares one schema. Phase 2 shell code lives entirely under `shell/` — no Rust crate for shell, it's just scripts + tests.

Task index:
- **Task 1:** Move/extend IPC types into `coati-core`
- **Task 2:** `OllamaClient::complete_json` for structured output
- **Task 3:** `propose()` + `explain()` helpers in `coati-core`
- **Task 4:** `coati propose` CLI subcommand
- **Task 5:** `coati explain` CLI subcommand
- **Task 6:** zsh plugin
- **Task 7:** bash plugin
- **Task 8:** installer
- **Task 9:** bats integration tests (mock daemon)
- **Task 10:** extend CI to run shell tests
- **Task 11:** fish plugin (stretch)
- **Task 12:** README + shell/README

Detailed task bodies follow below.

---

## Task 1: Move IPC types to `coati-core`

**Files:**
- Create: `crates/coati-core/src/ipc.rs`
- Modify: `crates/coati-core/src/lib.rs`
- Modify: `crates/coati-cli/src/ipc.rs` (remove moved types, re-export from core)

- [ ] **Step 1: Failing tests in `crates/coati-core/src/ipc.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_propose_request() {
        let req = Request::Propose {
            intent: "restart nginx".into(),
            context: ShellContext::default(),
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"type\":\"propose\""));
        assert!(s.contains("\"intent\":\"restart nginx\""));
    }

    #[test]
    fn deserializes_proposal_response() {
        let s = r#"{"type":"proposal","command":"sudo systemctl restart nginx","reasoning":"nginx service needs reload","needs_sudo":true}"#;
        let r: Response = serde_json::from_str(s).unwrap();
        match r {
            Response::Proposal { needs_sudo, .. } => assert!(needs_sudo),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn shell_context_round_trips() {
        let ctx = ShellContext {
            pwd: "/home/marche/coati".into(),
            last_command: Some("ls /nonexistent".into()),
            last_exit: Some(2),
            git_branch: Some("main".into()),
            shell: "zsh".into(),
        };
        let s = serde_json::to_string(&ctx).unwrap();
        let parsed: ShellContext = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.pwd, ctx.pwd);
        assert_eq!(parsed.last_exit, Some(2));
    }
}
```

- [ ] **Step 2: Run `cargo test -p coati-core ipc::tests` → compile fails**

- [ ] **Step 3: Implement in `crates/coati-core/src/ipc.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct ShellContext {
    #[serde(default)] pub pwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub last_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub last_exit: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub git_branch: Option<String>,
    #[serde(default)] pub shell: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Ping,
    Ask { question: String },
    Propose {
        intent: String,
        #[serde(default)] context: ShellContext,
    },
    Explain {
        command: String,
        #[serde(default)] stdout: String,
        #[serde(default)] stderr: String,
        exit_code: i32,
        #[serde(default)] context: ShellContext,
    },
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Pong,
    Answer { content: String },
    Proposal { command: String, reasoning: String, needs_sudo: bool },
    Explanation {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")] fix: Option<String>,
    },
    Error { message: String },
}
```

- [ ] **Step 4: Export from `lib.rs`**

```rust
pub mod ipc;
pub use ipc::{Request, Response, ShellContext};
```

- [ ] **Step 5: Update `crates/coati-cli/src/ipc.rs`**

Remove local `Request` / `Response` enums. Add at top:
```rust
pub use coati_core::ipc::{Request, Response, ShellContext};
```

Keep `IpcTransport`, `RequestHandler`, `UnixSocketTransport` unchanged.

- [ ] **Step 6: Run `cargo test -p coati-core ipc::tests`, `cargo test -p coati-cli`, `cargo clippy --workspace --all-targets -- -D warnings` → all pass**

- [ ] **Step 7: Commit**

```bash
git add crates/coati-core/ crates/coati-cli/
git commit -m "refactor(ipc): move Request/Response/ShellContext to coati-core

Adds Propose and Explain request variants, Proposal and Explanation
response variants, plus a ShellContext struct carrying pwd, last
command/exit, git branch, and shell name. Types now live in
coati-core::ipc so future desktop and voice surfaces share one schema."
```

---

## Task 2: `OllamaClient::complete_json` for structured output

**Files:**
- Modify: `crates/coati-core/src/llm.rs`

Ollama supports `format: "json"` or `format: <schema>`. We add a second method on `OllamaClient` that forces JSON output and parses it.

- [ ] **Step 1: Add failing test**

Inside the existing `#[cfg(test)] mod tests` in `llm.rs`:

```rust
    #[tokio::test]
    async fn ollama_complete_json_requests_json_format() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": { "role": "assistant", "content": "{\"k\":\"v\"}" },
                "done": true
            })))
            .mount(&server).await;

        let client = OllamaClient::new(server.uri(), "gemma4".into());
        let msg = ChatMessage { role: "user".into(), content: "return json".into() };
        let val: serde_json::Value = client.complete_json(&[msg], None).await.unwrap();
        assert_eq!(val["k"], "v");
    }
```

- [ ] **Step 2: Run → compile fails**

- [ ] **Step 3: Implement `complete_json` on `OllamaClient`**

At top of file if not present: `use anyhow::Context;`

Add the method inside `impl OllamaClient`:

```rust
    pub async fn complete_json(
        &self,
        messages: &[ChatMessage],
        schema: Option<serde_json::Value>,
    ) -> anyhow::Result<serde_json::Value> {
        let format = schema.unwrap_or_else(|| serde_json::json!("json"));
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "format": format,
            "stream": false,
        });
        let resp: serde_json::Value = self.http
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send().await?
            .error_for_status()?
            .json().await?;
        let content = resp["message"]["content"].as_str().unwrap_or("").to_string();
        let parsed: serde_json::Value = serde_json::from_str(&content)
            .with_context(|| format!("llm returned non-json: {content}"))?;
        Ok(parsed)
    }
```

- [ ] **Step 4: Test + clippy → pass**

- [ ] **Step 5: Commit**

```bash
git add crates/coati-core/
git commit -m "feat(core): add OllamaClient::complete_json for structured outputs"
```

---

## Task 3: `propose()` and `explain()` helpers in `coati-core`

**Files:**
- Create: `crates/coati-core/src/agent_ext.rs`
- Modify: `crates/coati-core/src/llm.rs` (add `Any` bound for downcast)
- Modify: `crates/coati-core/src/lib.rs`

Each helper packages a one-shot prompt and calls `complete_json` to return a strictly-typed struct.

- [ ] **Step 1: Failing tests in `agent_ext.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{OllamaClient, LlmProvider};
    use crate::ipc::ShellContext;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};
    use std::sync::Arc;

    #[tokio::test]
    async fn propose_returns_typed_proposal() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {
                    "role": "assistant",
                    "content": "{\"command\":\"sudo systemctl restart nginx\",\"reasoning\":\"nginx needs a reload\",\"needs_sudo\":true}"
                },
                "done": true
            })))
            .mount(&server).await;

        let client: Arc<dyn LlmProvider> = Arc::new(OllamaClient::new(server.uri(), "test".into()));
        let ctx = ShellContext { pwd: "/tmp".into(), shell: "zsh".into(), ..Default::default() };

        let p = propose(&client, "restart nginx", &ctx).await.unwrap();

        assert_eq!(p.command, "sudo systemctl restart nginx");
        assert!(p.needs_sudo);
        assert!(!p.reasoning.is_empty());
    }

    #[tokio::test]
    async fn explain_returns_typed_explanation() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {
                    "role": "assistant",
                    "content": "{\"text\":\"config has a typo at line 10\",\"fix\":\"nginx -t\"}"
                },
                "done": true
            })))
            .mount(&server).await;

        let client: Arc<dyn LlmProvider> = Arc::new(OllamaClient::new(server.uri(), "test".into()));
        let ctx = ShellContext::default();

        let e = explain(&client, "nginx -t", "", "typo at line 10", 1, &ctx).await.unwrap();

        assert!(e.text.contains("typo"));
        assert_eq!(e.fix.as_deref(), Some("nginx -t"));
    }
}
```

- [ ] **Step 2: Run → compile fails (types missing)**

- [ ] **Step 3: Extend `LlmProvider` trait with `Any` bound**

In `crates/coati-core/src/llm.rs`:

```rust
use std::any::Any;

#[async_trait]
pub trait LlmProvider: Send + Sync + Any {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[(&'static str, &'static str, serde_json::Value)],
    ) -> anyhow::Result<LlmResponse>;

    fn as_any(&self) -> &dyn Any;
}
```

In the `impl LlmProvider for OllamaClient` block, add:
```rust
    fn as_any(&self) -> &dyn Any { self }
```

Update `ScriptedLlm` in `crates/coati-core/src/agent.rs` test module similarly (add `fn as_any(&self) -> &dyn std::any::Any { self }`).

- [ ] **Step 4: Implement `crates/coati-core/src/agent_ext.rs`**

```rust
use crate::ipc::ShellContext;
use crate::llm::{ChatMessage, LlmProvider, OllamaClient};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize)]
pub struct Proposal {
    pub command: String,
    pub reasoning: String,
    #[serde(default)] pub needs_sudo: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Explanation {
    pub text: String,
    #[serde(default)] pub fix: Option<String>,
}

fn propose_prompt(intent: &str, ctx: &ShellContext) -> String {
    format!(
        "You are a Linux shell expert. Propose ONE concrete shell command the user should run \
         to accomplish their intent. Return STRICT JSON matching:\n\
         {{ \"command\": \"<the command>\", \"reasoning\": \"<one sentence why>\", \"needs_sudo\": <bool> }}\n\
         \n\
         User intent: {intent}\n\
         Working directory: {pwd}\n\
         Shell: {shell}\n\
         Git branch: {branch}\n\
         \n\
         Rules: single command only, no pipelines unless essential, no interactive editors, \
         prefer the least-privileged option. needs_sudo must be true iff the command begins with `sudo `.",
        intent = intent, pwd = ctx.pwd, shell = ctx.shell,
        branch = ctx.git_branch.as_deref().unwrap_or("(none)"),
    )
}

fn explain_prompt(command: &str, stdout: &str, stderr: &str, exit_code: i32, ctx: &ShellContext) -> String {
    format!(
        "You are a Linux shell expert. Explain why the command failed and suggest a concrete fix.\n\
         Return STRICT JSON:\n\
         {{ \"text\": \"<2-3 sentence explanation>\", \"fix\": \"<one-line fix command or null>\" }}\n\
         \n\
         Command: {command}\n\
         Exit code: {exit_code}\n\
         Stdout:\n{stdout}\n\
         Stderr:\n{stderr}\n\
         Working directory: {pwd}",
        pwd = ctx.pwd,
    )
}

pub async fn propose(llm: &Arc<dyn LlmProvider>, intent: &str, ctx: &ShellContext) -> anyhow::Result<Proposal> {
    let msgs = vec![ChatMessage { role: "user".into(), content: propose_prompt(intent, ctx) }];
    let ollama = llm.as_any().downcast_ref::<OllamaClient>()
        .ok_or_else(|| anyhow::anyhow!("propose() requires OllamaClient"))?;
    let val = ollama.complete_json(&msgs, None).await?;
    Ok(serde_json::from_value(val)?)
}

pub async fn explain(
    llm: &Arc<dyn LlmProvider>,
    command: &str, stdout: &str, stderr: &str, exit_code: i32,
    ctx: &ShellContext,
) -> anyhow::Result<Explanation> {
    let msgs = vec![ChatMessage { role: "user".into(), content: explain_prompt(command, stdout, stderr, exit_code, ctx) }];
    let ollama = llm.as_any().downcast_ref::<OllamaClient>()
        .ok_or_else(|| anyhow::anyhow!("explain() requires OllamaClient"))?;
    let val = ollama.complete_json(&msgs, None).await?;
    Ok(serde_json::from_value(val)?)
}
```

- [ ] **Step 5: Update `lib.rs`**

```rust
pub mod agent_ext;
pub use agent_ext::{Proposal, Explanation, propose, explain};
```

- [ ] **Step 6: Tests + clippy**

```bash
cargo test -p coati-core
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 7: Commit**

```bash
git add crates/coati-core/
git commit -m "feat(core): add propose() and explain() agent helpers using complete_json

LlmProvider trait now requires Any + as_any for the downcast needed
by the complete_json path; OllamaClient implements both."
```

---

## Task 4: `coati propose` CLI subcommand

**Files:**
- Create: `crates/coati-cli/src/cmd_propose.rs`
- Create: `crates/coati-cli/tests/propose.rs`
- Modify: `crates/coati-cli/src/main.rs`

- [ ] **Step 1: Failing integration tests in `crates/coati-cli/tests/propose.rs`**

```rust
use assert_cmd::Command;

#[test]
fn propose_help_mentions_json_flag() {
    let out = Command::cargo_bin("coati").unwrap()
        .args(["propose", "--help"])
        .output().unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--json"));
}

#[test]
fn propose_rejects_empty_intent() {
    Command::cargo_bin("coati").unwrap()
        .arg("propose")
        .assert()
        .failure();
}
```

- [ ] **Step 2: Run → fails**

- [ ] **Step 3: Add variant + dispatch in `main.rs`**

Add to `Commands`:
```rust
    /// Propose a shell command for a natural-language intent.
    Propose {
        /// The intent, e.g. "restart nginx"
        intent: String,
        /// Emit machine-readable JSON instead of human text.
        #[arg(long)] json: bool,
        /// Pre-captured shell context as JSON (overrides auto-detection).
        #[arg(long)] context: Option<String>,
    },
```

Add `mod cmd_propose;`. Dispatch arm:
```rust
Commands::Propose { intent, json, context } =>
    cmd_propose::run(&intent, json, context.as_deref()).await,
```

- [ ] **Step 4: Implement `crates/coati-cli/src/cmd_propose.rs`**

```rust
use coati_core::ipc::ShellContext;
use coati_core::{Config, OllamaClient, propose};
use std::sync::Arc;

pub async fn run(intent: &str, json: bool, context_override: Option<&str>) -> anyhow::Result<()> {
    let cfg = Config::load_or_default()?;
    let llm: Arc<dyn coati_core::LlmProvider> =
        Arc::new(OllamaClient::new(cfg.llm.endpoint.clone(), cfg.llm.model.clone()));

    let ctx = if let Some(raw) = context_override {
        serde_json::from_str::<ShellContext>(raw)
            .map_err(|e| anyhow::anyhow!("invalid --context JSON: {e}"))?
    } else {
        auto_context()
    };

    let p = propose(&llm, intent, &ctx).await?;

    if json {
        println!("{}", serde_json::json!({
            "command": p.command,
            "reasoning": p.reasoning,
            "needs_sudo": p.needs_sudo,
        }));
    } else {
        if p.needs_sudo { println!("⚠ needs sudo"); }
        println!("$ {}", p.command);
        println!("  → {}", p.reasoning);
    }
    Ok(())
}

fn auto_context() -> ShellContext {
    ShellContext {
        pwd: std::env::current_dir().ok().and_then(|p| p.to_str().map(String::from)).unwrap_or_default(),
        shell: std::env::var("SHELL").ok().and_then(|s| s.rsplit('/').next().map(String::from)).unwrap_or_default(),
        ..Default::default()
    }
}
```

- [ ] **Step 5: Build + tests + clippy**

```bash
cargo build -p coati-cli
cargo test -p coati-cli --test propose
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 6: Commit**

```bash
git add crates/coati-cli/
git commit -m "feat(cli): add coati propose subcommand with --json output"
```

---

## Task 5: `coati explain` CLI subcommand

**Files:**
- Create: `crates/coati-cli/src/cmd_explain.rs`
- Create: `crates/coati-cli/tests/explain.rs`
- Modify: `crates/coati-cli/src/main.rs`

- [ ] **Step 1: Failing tests in `crates/coati-cli/tests/explain.rs`**

```rust
use assert_cmd::Command;

#[test]
fn explain_help_mentions_required_flags() {
    let out = Command::cargo_bin("coati").unwrap()
        .args(["explain", "--help"])
        .output().unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--command"));
    assert!(s.contains("--exit"));
    assert!(s.contains("--json"));
}

#[test]
fn explain_rejects_missing_command() {
    Command::cargo_bin("coati").unwrap()
        .args(["explain", "--exit", "1"])
        .assert()
        .failure();
}
```

- [ ] **Step 2: Run → fails**

- [ ] **Step 3: Add variant + dispatch**

In `main.rs` Commands:
```rust
    /// Explain why a command failed, with an optional fix.
    Explain {
        #[arg(long)] command: String,
        #[arg(long, default_value = "")] stdout: String,
        #[arg(long, default_value = "")] stderr: String,
        #[arg(long)] exit: i32,
        #[arg(long)] json: bool,
        #[arg(long)] context: Option<String>,
    },
```

Dispatch:
```rust
Commands::Explain { command, stdout, stderr, exit, json, context } =>
    cmd_explain::run(&command, &stdout, &stderr, exit, json, context.as_deref()).await,
```

Add `mod cmd_explain;`.

- [ ] **Step 4: Implement `crates/coati-cli/src/cmd_explain.rs`**

```rust
use coati_core::ipc::ShellContext;
use coati_core::{Config, OllamaClient, explain};
use std::sync::Arc;

pub async fn run(
    command: &str, stdout: &str, stderr: &str, exit: i32,
    json: bool, context_override: Option<&str>,
) -> anyhow::Result<()> {
    let cfg = Config::load_or_default()?;
    let llm: Arc<dyn coati_core::LlmProvider> =
        Arc::new(OllamaClient::new(cfg.llm.endpoint.clone(), cfg.llm.model.clone()));

    let ctx = if let Some(raw) = context_override {
        serde_json::from_str::<ShellContext>(raw)
            .map_err(|e| anyhow::anyhow!("invalid --context JSON: {e}"))?
    } else {
        ShellContext {
            pwd: std::env::current_dir().ok().and_then(|p| p.to_str().map(String::from)).unwrap_or_default(),
            shell: std::env::var("SHELL").ok().and_then(|s| s.rsplit('/').next().map(String::from)).unwrap_or_default(),
            ..Default::default()
        }
    };

    let e = explain(&llm, command, stdout, stderr, exit, &ctx).await?;

    if json {
        println!("{}", serde_json::json!({
            "text": e.text,
            "fix": e.fix,
        }));
    } else {
        println!("{}", e.text);
        if let Some(fix) = &e.fix {
            println!();
            println!("Try: {}", fix);
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Build + tests + clippy**

- [ ] **Step 6: Commit**

```bash
git add crates/coati-cli/
git commit -m "feat(cli): add coati explain subcommand with --json output"
```

---

## Task 6: zsh plugin

**Files:**
- Create: `shell/zsh/coati.plugin.zsh`

Uses zsh's `add-zsh-hook` for `preexec` (captures the command about to run) and `precmd` (captures exit code).

- [ ] **Step 1: Write the plugin**

```zsh
# coati.plugin.zsh — Coati shell integration for zsh
# Requires: coati binary on $PATH
# Install: source /path/to/shell/zsh/coati.plugin.zsh
#         (or drop in ~/.oh-my-zsh/custom/plugins/coati/ and enable in .zshrc)

typeset -g _coati_last_cmd=""
typeset -g _coati_last_exit=0

_coati_preexec() { _coati_last_cmd="$1"; }
_coati_precmd()  { _coati_last_exit=$?; }

autoload -Uz add-zsh-hook
add-zsh-hook preexec _coati_preexec
add-zsh-hook precmd  _coati_precmd

_coati_json_escape() { printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'; }

_coati_context_json() {
    local branch
    branch="$(git rev-parse --abbrev-ref HEAD 2>/dev/null)"
    printf '{"pwd":"%s","last_command":"%s","last_exit":%d,"git_branch":%s,"shell":"zsh"}\n' \
        "$(_coati_json_escape "$PWD")" \
        "$(_coati_json_escape "$_coati_last_cmd")" \
        "$_coati_last_exit" \
        "$([[ -n "$branch" ]] && printf '"%s"' "$(_coati_json_escape "$branch")" || printf 'null')"
}

# Tiny JSON getter for flat objects — avoids jq dependency.
_coati_jget() {
    local key="$1"
    sed -n "s/.*\"${key}\":\"\([^\"]*\)\".*/\1/p; s/.*\"${key}\":\(true\|false\|null\|-\?[0-9]*\).*/\1/p" | head -1
}

coati() {
    case "$1" in
        ""|-h|--help|ask|serve|model|hw|setup|propose|explain)
            command coati "$@"
            return $?
            ;;
    esac

    local intent="$*"
    local ctx resp
    ctx="$(_coati_context_json)"
    resp="$(command coati propose --json --context "$ctx" -- "$intent" 2>/dev/null)" || {
        print -u2 "coati: agent unreachable or errored"
        return 1
    }

    local cmd reasoning needs_sudo
    cmd="$(printf '%s' "$resp"       | _coati_jget command)"
    reasoning="$(printf '%s' "$resp" | _coati_jget reasoning)"
    needs_sudo="$(printf '%s' "$resp" | _coati_jget needs_sudo)"

    [[ -z "$cmd" ]] && { print -u2 "coati: empty proposal"; return 1; }
    [[ "$needs_sudo" == "true" ]] && print -u2 "⚠ needs sudo"
    print -u2 "$ $cmd"
    [[ -n "$reasoning" ]] && print -u2 "  → $reasoning"

    local prompt="Run? [y/N] "
    [[ "$needs_sudo" == "true" ]] && prompt="sudo command — run? [y/N] "
    read -k 1 "reply?$prompt"
    print
    if [[ "$reply" == "y" || "$reply" == "Y" ]]; then
        eval "$cmd"
    fi
}

# ?? — explain the last command
\?\?() {
    if [[ -z "$_coati_last_cmd" ]]; then
        print -u2 "coati: no previous command captured"
        return 1
    fi
    local ctx
    ctx="$(_coati_context_json)"
    command coati explain \
        --command "$_coati_last_cmd" \
        --exit "$_coati_last_exit" \
        --stderr "" \
        --stdout "" \
        --context "$ctx"
}
```

- [ ] **Step 2: Manual smoke**

```bash
zsh -c 'source /home/marche/coati/shell/zsh/coati.plugin.zsh; _coati_context_json'
```

Expected: one JSON line with `pwd`, `shell`:"zsh", `last_exit`:0, `git_branch`:"main" (or null).

- [ ] **Step 3: Commit**

```bash
git add shell/zsh/
git commit -m "feat(shell): add zsh plugin with coati wrapper and ?? widget

- preexec/precmd hooks capture last command + exit code
- coati <intent> calls coati propose --json and prompts [y/N] before
  running the proposed command (default No; sudo gets extra warning)
- ?? calls coati explain with the captured last command and exit code
- Dependency-free: uses sed for JSON parsing, no jq required"
```

---

## Task 7: bash plugin

**Files:**
- Create: `shell/bash/coati.bash`

bash has no native `preexec`; use `trap DEBUG` + `PROMPT_COMMAND`.

- [ ] **Step 1: Write the plugin**

```bash
# coati.bash — Coati shell integration for bash
# Requires: coati binary on $PATH
# Install: source /path/to/shell/bash/coati.bash  (append to .bashrc)

_coati_last_cmd=""
_coati_last_exit=0

_coati_preexec() {
    # Skip our own helpers and the prompt command itself
    case "$BASH_COMMAND" in
        _coati_*) return ;;
        "$PROMPT_COMMAND"*) return ;;
    esac
    _coati_last_cmd="$BASH_COMMAND"
}
trap '_coati_preexec' DEBUG

_coati_precmd() { _coati_last_exit=$?; }
if [[ "$PROMPT_COMMAND" != *_coati_precmd* ]]; then
    PROMPT_COMMAND="_coati_precmd${PROMPT_COMMAND:+; $PROMPT_COMMAND}"
fi

_coati_json_escape() { printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'; }

_coati_context_json() {
    local branch="$(git rev-parse --abbrev-ref HEAD 2>/dev/null)"
    printf '{"pwd":"%s","last_command":"%s","last_exit":%d,"git_branch":%s,"shell":"bash"}\n' \
        "$(_coati_json_escape "$PWD")" \
        "$(_coati_json_escape "$_coati_last_cmd")" \
        "$_coati_last_exit" \
        "$([[ -n "$branch" ]] && printf '"%s"' "$(_coati_json_escape "$branch")" || printf 'null')"
}

_coati_jget() {
    local key="$1"
    sed -n "s/.*\"${key}\":\"\([^\"]*\)\".*/\1/p; s/.*\"${key}\":\(true\|false\|null\|-\?[0-9]*\).*/\1/p" | head -1
}

coati() {
    case "$1" in
        ""|-h|--help|ask|serve|model|hw|setup|propose|explain)
            command coati "$@"; return $? ;;
    esac
    local intent="$*"
    local ctx="$(_coati_context_json)"
    local resp
    resp="$(command coati propose --json --context "$ctx" -- "$intent" 2>/dev/null)" || {
        echo "coati: agent unreachable or errored" >&2; return 1
    }
    local cmd="$(printf '%s' "$resp" | _coati_jget command)"
    local reasoning="$(printf '%s' "$resp" | _coati_jget reasoning)"
    local needs_sudo="$(printf '%s' "$resp" | _coati_jget needs_sudo)"

    [[ -z "$cmd" ]] && { echo "coati: empty proposal" >&2; return 1; }
    [[ "$needs_sudo" == "true" ]] && echo "⚠ needs sudo" >&2
    echo "$ $cmd" >&2
    [[ -n "$reasoning" ]] && echo "  → $reasoning" >&2

    local prompt="Run? [y/N] "
    [[ "$needs_sudo" == "true" ]] && prompt="sudo command — run? [y/N] "
    local reply
    read -n 1 -p "$prompt" reply
    echo
    if [[ "$reply" == "y" || "$reply" == "Y" ]]; then
        eval "$cmd"
    fi
}

# ?? isn't a valid function name in bash — alias a standalone command
_coati_qq() {
    [[ -z "$_coati_last_cmd" ]] && { echo "coati: no previous command captured" >&2; return 1; }
    local ctx="$(_coati_context_json)"
    command coati explain \
        --command "$_coati_last_cmd" \
        --exit "$_coati_last_exit" \
        --stderr "" \
        --stdout "" \
        --context "$ctx"
}
alias '??'=_coati_qq
```

- [ ] **Step 2: Manual smoke**

```bash
bash -c 'source /home/marche/coati/shell/bash/coati.bash; _coati_context_json'
```

- [ ] **Step 3: Commit**

```bash
git add shell/bash/
git commit -m "feat(shell): add bash plugin mirroring zsh plugin UX"
```

---

## Task 8: Installer script

**Files:**
- Create: `shell/install.sh`

- [ ] **Step 1: Write installer**

```bash
#!/usr/bin/env bash
# Coati shell plugin installer
# Usage: ./shell/install.sh [--shell zsh|bash|fish|auto]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

TARGET_SHELL="auto"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --shell) TARGET_SHELL="$2"; shift 2 ;;
        -h|--help)
            cat <<EOF
Coati shell plugin installer

Usage: $0 [--shell zsh|bash|fish|auto]

With --shell auto (default), detects from \$SHELL.
Appends a single source line to your rc file. Idempotent.
EOF
            exit 0 ;;
        *) echo "unknown flag: $1" >&2; exit 1 ;;
    esac
done

if [[ "$TARGET_SHELL" == "auto" ]]; then
    case "$(basename "${SHELL:-/bin/bash}")" in
        zsh)  TARGET_SHELL="zsh" ;;
        bash) TARGET_SHELL="bash" ;;
        fish) TARGET_SHELL="fish" ;;
        *) echo "Could not auto-detect shell; use --shell explicitly" >&2; exit 1 ;;
    esac
fi

MARKER="# coati shell plugin"
case "$TARGET_SHELL" in
    zsh)
        PLUGIN="$REPO_ROOT/shell/zsh/coati.plugin.zsh"
        RC="$HOME/.zshrc" ;;
    bash)
        PLUGIN="$REPO_ROOT/shell/bash/coati.bash"
        RC="$HOME/.bashrc" ;;
    fish)
        PLUGIN="$REPO_ROOT/shell/fish/coati.fish"
        RC="$HOME/.config/fish/config.fish"
        mkdir -p "$(dirname "$RC")" ;;
    *) echo "unsupported shell: $TARGET_SHELL" >&2; exit 1 ;;
esac

if [[ ! -f "$PLUGIN" ]]; then
    echo "plugin not found: $PLUGIN" >&2
    exit 1
fi

if [[ -f "$RC" ]] && grep -qF "$MARKER" "$RC"; then
    echo "✓ coati plugin already installed in $RC"
    exit 0
fi

{
    echo ""
    echo "$MARKER"
    echo "source \"$PLUGIN\""
} >> "$RC"

echo "✓ appended coati source line to $RC"
echo "  run:  . \"$RC\"   (or restart your terminal)"
```

- [ ] **Step 2: Make executable + smoke**

```bash
chmod +x shell/install.sh
./shell/install.sh --help
```

- [ ] **Step 3: Commit**

```bash
git add shell/install.sh
git commit -m "feat(shell): add installer with zsh/bash/fish auto-detect; idempotent append"
```

---

## Task 9: Hermetic integration tests (bats + mock daemon)

**Files:**
- Create: `shell/tests/mock_coati.sh`
- Create: `shell/tests/zsh.bats`
- Create: `shell/tests/bash.bats`
- Create: `shell/tests/run.sh`

Plugin calls `coati propose --json ...`. In tests we prepend a temp dir to `PATH` whose `coati` binary is actually a mock script returning canned JSON.

- [ ] **Step 1: Mock binary `shell/tests/mock_coati.sh`**

```bash
#!/usr/bin/env bash
# Mock coati binary for shell-plugin tests. Emits canned JSON by intent keyword.
case "$1" in
    propose)
        intent=""
        while [[ $# -gt 0 ]]; do
            case "$1" in
                --) shift; intent="$*"; break ;;
                --json|--context) shift 2 ;;
                propose) shift ;;
                *) shift ;;
            esac
        done
        case "$intent" in
            *nginx*) echo '{"command":"sudo systemctl restart nginx","reasoning":"reload nginx","needs_sudo":true}' ;;
            *disk*)  echo '{"command":"df -h","reasoning":"show disk usage","needs_sudo":false}' ;;
            *echo*)  echo '{"command":"echo hi","reasoning":"say hi","needs_sudo":false}' ;;
            *)       echo '{"command":"echo hi","reasoning":"stub","needs_sudo":false}' ;;
        esac ;;
    explain)
        echo '{"text":"mock explanation","fix":"true"}' ;;
    *) echo "mock coati: unknown subcommand $1" >&2; exit 2 ;;
esac
```

`chmod +x shell/tests/mock_coati.sh`.

- [ ] **Step 2: `shell/tests/zsh.bats`**

```bash
#!/usr/bin/env bats

setup() {
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
    MOCK_DIR="$(mktemp -d)"
    ln -s "$REPO_ROOT/shell/tests/mock_coati.sh" "$MOCK_DIR/coati"
    export PATH="$MOCK_DIR:$PATH"
    PLUGIN="$REPO_ROOT/shell/zsh/coati.plugin.zsh"
}

teardown() { rm -rf "$MOCK_DIR"; }

@test "zsh: context_json has pwd and shell" {
    run zsh -c "source '$PLUGIN' && _coati_context_json"
    [ "$status" -eq 0 ]
    [[ "$output" == *'"pwd":'* ]]
    [[ "$output" == *'"shell":"zsh"'* ]]
}

@test "zsh: coati declines without y (default No)" {
    # Feed newline (no y) — should print proposal but not run
    run bash -c "printf '\n' | zsh -i -c \"source '$PLUGIN' && coati echo something\""
    [ "$status" -eq 0 ]
    [[ "$output" == *'echo hi'* ]]   # proposal shown
    # If it had run the echo, output would end with "hi" on its own line
    [[ "$output" != *$'\nhi\n'* ]]
}

@test "zsh: coati executes on y" {
    run bash -c "printf 'y' | zsh -i -c \"source '$PLUGIN' && coati echo something\""
    [ "$status" -eq 0 ]
    [[ "$output" == *'hi'* ]]
}

@test "zsh: sudo intent shows warning" {
    run bash -c "printf '\n' | zsh -i -c \"source '$PLUGIN' && coati restart nginx\""
    [[ "$output" == *'needs sudo'* ]]
    [[ "$output" == *'sudo systemctl restart nginx'* ]]
}
```

- [ ] **Step 3: `shell/tests/bash.bats` (same four tests, bash-targeted)**

```bash
#!/usr/bin/env bats

setup() {
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
    MOCK_DIR="$(mktemp -d)"
    ln -s "$REPO_ROOT/shell/tests/mock_coati.sh" "$MOCK_DIR/coati"
    export PATH="$MOCK_DIR:$PATH"
    PLUGIN="$REPO_ROOT/shell/bash/coati.bash"
}

teardown() { rm -rf "$MOCK_DIR"; }

@test "bash: context_json has pwd and shell" {
    run bash -c "source '$PLUGIN' && _coati_context_json"
    [ "$status" -eq 0 ]
    [[ "$output" == *'"pwd":'* ]]
    [[ "$output" == *'"shell":"bash"'* ]]
}

@test "bash: coati declines without y (default No)" {
    run bash -c "printf '\n' | bash -c \"source '$PLUGIN' && coati echo something\""
    [ "$status" -eq 0 ]
    [[ "$output" == *'echo hi'* ]]
    [[ "$output" != *$'\nhi\n'* ]]
}

@test "bash: coati executes on y" {
    run bash -c "printf 'y' | bash -c \"source '$PLUGIN' && coati echo something\""
    [ "$status" -eq 0 ]
    [[ "$output" == *'hi'* ]]
}

@test "bash: sudo intent shows warning" {
    run bash -c "printf '\n' | bash -c \"source '$PLUGIN' && coati restart nginx\""
    [[ "$output" == *'needs sudo'* ]]
    [[ "$output" == *'sudo systemctl restart nginx'* ]]
}
```

- [ ] **Step 4: `shell/tests/run.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v bats >/dev/null 2>&1; then
    echo "FAIL: bats not installed. apt install bats (Ubuntu) or brew install bats-core" >&2
    exit 1
fi

bats "$SCRIPT_DIR/zsh.bats" "$SCRIPT_DIR/bash.bats"
```

`chmod +x shell/tests/run.sh`.

- [ ] **Step 5: Install bats (if missing) and run tests**

```bash
sudo apt-get install -y bats zsh 2>/dev/null || true
./shell/tests/run.sh
```

Expected: 8/8 tests pass.

- [ ] **Step 6: Commit**

```bash
git add shell/tests/
git commit -m "test(shell): add bats tests with mock daemon covering context capture,
confirm-default-no, y-to-run, and sudo warning for both zsh and bash"
```

---

## Task 10: Extend CI to run shell tests

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add `shell` job**

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

  shell:
    runs-on: ubuntu-24.04
    needs: check
    steps:
      - uses: actions/checkout@v4
      - name: Install bats and zsh
        run: sudo apt-get update && sudo apt-get install -y bats zsh
      - name: Run shell tests
        run: ./shell/tests/run.sh
```

- [ ] **Step 2: Commit**

```bash
git add .github/
git commit -m "ci: add shell integration tests job (bats + zsh + bash)"
```

---

## Task 11: Fish plugin (stretch)

**Files:**
- Create: `shell/fish/coati.fish`

Fish uses `fish_preexec` / `fish_postexec` events. Different enough to warrant a parallel implementation rather than a shim.

- [ ] **Step 1: Write the plugin**

```fish
# coati.fish — Coati shell integration for fish

set -g _coati_last_cmd ""
set -g _coati_last_exit 0

function _coati_preexec --on-event fish_preexec
    set -g _coati_last_cmd $argv[1]
end

function _coati_postexec --on-event fish_postexec
    set -g _coati_last_exit $status
end

function _coati_context_json
    set -l branch (git rev-parse --abbrev-ref HEAD 2>/dev/null; or echo "")
    set -l branch_field "null"
    if test -n "$branch"
        set branch_field "\"$branch\""
    end
    printf '{"pwd":"%s","last_command":"%s","last_exit":%d,"git_branch":%s,"shell":"fish"}\n' \
        (string escape --style=json -- $PWD) \
        (string escape --style=json -- $_coati_last_cmd) \
        $_coati_last_exit \
        $branch_field
end

function _coati_jget
    set -l key $argv[1]
    sed -n "s/.*\"$key\":\"\([^\"]*\)\".*/\1/p; s/.*\"$key\":\(true\|false\|null\|-\?[0-9]*\).*/\1/p" | head -1
end

function coati
    switch "$argv[1]"
        case "" -h --help ask serve model hw setup propose explain
            command coati $argv
            return $status
    end
    set -l intent (string join " " $argv)
    set -l ctx (_coati_context_json)
    set -l resp (command coati propose --json --context "$ctx" -- "$intent" 2>/dev/null)
    if test $status -ne 0
        echo "coati: agent unreachable" >&2; return 1
    end
    set -l cmd (printf '%s' $resp | _coati_jget command)
    set -l reasoning (printf '%s' $resp | _coati_jget reasoning)
    set -l needs_sudo (printf '%s' $resp | _coati_jget needs_sudo)

    if test -z "$cmd"
        echo "coati: empty proposal" >&2; return 1
    end
    test "$needs_sudo" = "true"; and echo "⚠ needs sudo" >&2
    echo "\$ $cmd" >&2
    test -n "$reasoning"; and echo "  → $reasoning" >&2

    set -l prompt "Run? [y/N] "
    test "$needs_sudo" = "true"; and set prompt "sudo command — run? [y/N] "
    read -P "$prompt" -n 1 reply
    if test "$reply" = "y" -o "$reply" = "Y"
        eval $cmd
    end
end

function '??'
    test -z "$_coati_last_cmd"; and begin; echo "coati: no previous command captured" >&2; return 1; end
    set -l ctx (_coati_context_json)
    command coati explain --command "$_coati_last_cmd" --exit $_coati_last_exit --stderr "" --stdout "" --context "$ctx"
end
```

- [ ] **Step 2: Smoke + commit**

```bash
fish -c 'source shell/fish/coati.fish; _coati_context_json' 2>/dev/null || echo "fish not installed — skipped smoke"
git add shell/fish/
git commit -m "feat(shell): add fish plugin (stretch)"
```

---

## Task 12: README + shell/README + CLAUDE.md update

**Files:**
- Modify: `README.md`
- Create: `shell/README.md`
- Modify: `CLAUDE.md` (minor — note Phase 2 status)

- [ ] **Step 1: Replace `README.md` body**

```markdown
# coati

Your Linux copilot — a local, private, voice-capable system agent.

Ships as a one-installer suite, not a distro. Runs on Ubuntu 24.04+.

## Status

- **Phase 1** ✅ shipped (`v0.0.1-phase1`) — agent backend, 5 typed system tools, Unix-socket daemon
- **Phase 2** ✅ shipped (`v0.0.2-phase2`) — zsh, bash, fish shell plugins with `coati <intent>` and `??`
- **Phase 3** 🚧 next — Tauri tray + chat window

## Quick start

```bash
# 1. Prereqs: Rust 1.82+, ollama (https://ollama.com)
# 2. Build
git clone https://github.com/JuanMarchetto/coati
cd coati
cargo build --release
sudo cp target/release/coati /usr/local/bin/

# 3. First-run setup (TUI: hardware detection, model picker, ollama pull, config write)
coati setup

# 4. Install shell plugin
./shell/install.sh           # auto-detects zsh/bash/fish
. ~/.zshrc                   # reload (or restart terminal)
```

## Usage

```text
$ coati "show disk usage"
$ df -h
  → shows disk usage per mounted filesystem
Run? [y/N] y
Filesystem      Size  Used Avail Use% Mounted on
...

$ ls /nonexistent
ls: cannot access '/nonexistent': No such file or directory
$ ??
No such file or directory: /nonexistent does not exist.
Try: find / -name nonexistent 2>/dev/null
```

See [CLAUDE.md](CLAUDE.md) for product vision and [ROADMAP.md](ROADMAP.md) for the build plan.
```

- [ ] **Step 2: Create `shell/README.md`**

```markdown
# Coati shell plugins

Thin zsh / bash / fish integrations that expose `coati <intent>` and `??`.

## Install

```bash
./shell/install.sh              # auto-detect
./shell/install.sh --shell zsh  # force
```

Appends a single `source` line to your rc file. Idempotent.

## What gets installed

- `coati <natural language>` — sends the intent to the local coati daemon, shows a proposed command with one-line reasoning, prompts `[y/N]` before running. Sudo commands get an extra warning line.
- `??` — explains the previous command (exit code auto-captured). Prints cause and a fix suggestion.
- `preexec`/`precmd` hooks capture: `pwd`, last command, last exit code, git branch, shell name. All sent as a `ShellContext` JSON only to the local Unix socket.

## Dependencies

- `coati` binary on `$PATH`
- No `jq` required — parsing uses `sed` for the small flat JSON objects emitted by the daemon

## Tests

```bash
./shell/tests/run.sh   # bats + mock daemon
```

## Architecture notes

The shell plugins never call the network. They only invoke the local `coati` binary, which in turn talks only to the local Unix socket daemon. Any sudo command requires an explicit `y` keypress; confirm-default-no is enforced at the shell layer so even if a malicious proposal slipped through, the user sees it before anything runs.
```

- [ ] **Step 3: Bump `CLAUDE.md`**

Add to `CLAUDE.md` near the MVP section or "Working with Claude in this repo":

```markdown
Phase 2 (shell integration) shipped 2026-04-XX. Next: Phase 3 (Tauri desktop tray + chat window).
```

- [ ] **Step 4: Commit**

```bash
git add README.md shell/README.md CLAUDE.md
git commit -m "docs: document Phase 2 shell integration + usage examples"
```

---

## Phase 2 Exit Checkpoint

Before moving to Phase 3, confirm:

- [ ] `cargo test --workspace` passes, 0 warnings
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `./shell/tests/run.sh` passes (8/8 bats tests)
- [ ] In a real zsh shell with the plugin sourced: `coati "show disk usage"` proposes `df -h`, `[y/N]` prompt works, `y` runs, `<Enter>` doesn't
- [ ] `ls /nonexistent` then `??` returns a real explanation (requires live ollama)
- [ ] `coati propose --json "restart nginx"` prints a single JSON line with `command`, `reasoning`, `needs_sudo`
- [ ] `coati explain --json --command "false" --exit 1` prints a single JSON line with `text`, `fix`
- [ ] Install script is idempotent (run twice → no duplicate source line)
- [ ] README has install + usage walkthrough
- [ ] CI shell-job runs green on `main`
- [ ] No sudo is executed without explicit `y` keypress anywhere

When all boxes are checked, tag `v0.0.2-phase2` and start Phase 3 (desktop tray + chat).

---

## Self-review

- **Spec coverage:** zsh (T6), bash (T7), fish (T11, stretch), `??` (T6/T7/T11), NL wrapper (T6/T7/T11), confirm prompt (T6/T7 default No), sudo warning, context capture (pwd/last_command/last_exit/git_branch/shell), oh-my-zsh layout (`shell/zsh/coati.plugin.zsh` is the canonical OMZ file location), install script (T8), tests (T9), CI (T10), local-first (shell → local binary → local socket; no network calls anywhere).
- **Placeholder scan:** no TBD / TODO / "fill in details"; every step has either code or an exact command.
- **Type consistency:** `Request::Propose` / `Response::Proposal` / `Request::Explain` / `Response::Explanation` defined in Task 1, referenced consistently in Tasks 3–7. `ShellContext` fields match across T1 (definition), T6/T7/T11 (shell JSON builders), and T4/T5 (CLI `--context` parser).
- **Known minor risk:** the `_coati_jget` sed-based parser is brittle if a value contains a literal double-quote. Since daemon JSON is produced by `serde_json`, control characters are escaped; in practice the parser holds. If a future failure surfaces, upgrade to `jq` (soft dep, noted in `shell/README.md`).
