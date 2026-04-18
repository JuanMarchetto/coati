use crate::ipc::ShellContext;
use crate::llm::{ChatMessage, LlmProvider, OllamaClient};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize)]
pub struct Proposal {
    pub command: String,
    pub reasoning: String,
    #[serde(default)]
    pub needs_sudo: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Explanation {
    pub text: String,
    #[serde(default)]
    pub fix: Option<String>,
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

fn explain_prompt(
    command: &str,
    stdout: &str,
    stderr: &str,
    exit_code: i32,
    ctx: &ShellContext,
) -> String {
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

pub async fn propose(
    llm: &Arc<dyn LlmProvider>,
    intent: &str,
    ctx: &ShellContext,
) -> anyhow::Result<Proposal> {
    let msgs = vec![ChatMessage {
        role: "user".into(),
        content: propose_prompt(intent, ctx),
    }];
    let ollama = llm
        .as_any()
        .downcast_ref::<OllamaClient>()
        .ok_or_else(|| anyhow::anyhow!("propose() requires OllamaClient"))?;
    let val = ollama.complete_json(&msgs, None).await?;
    Ok(serde_json::from_value(val)?)
}

pub async fn explain(
    llm: &Arc<dyn LlmProvider>,
    command: &str,
    stdout: &str,
    stderr: &str,
    exit_code: i32,
    ctx: &ShellContext,
) -> anyhow::Result<Explanation> {
    let msgs = vec![ChatMessage {
        role: "user".into(),
        content: explain_prompt(command, stdout, stderr, exit_code, ctx),
    }];
    let ollama = llm
        .as_any()
        .downcast_ref::<OllamaClient>()
        .ok_or_else(|| anyhow::anyhow!("explain() requires OllamaClient"))?;
    let val = ollama.complete_json(&msgs, None).await?;
    Ok(serde_json::from_value(val)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::ShellContext;
    use crate::llm::OllamaClient;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
        let ctx = ShellContext {
            pwd: "/tmp".into(),
            shell: "zsh".into(),
            ..Default::default()
        };

        let p = propose(&client, "restart nginx", &ctx).await.unwrap();

        assert_eq!(p.command, "sudo systemctl restart nginx");
        assert!(p.needs_sudo);
        assert!(!p.reasoning.is_empty());
    }

    #[tokio::test]
    async fn explain_returns_typed_explanation() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {
                    "role": "assistant",
                    "content": "{\"text\":\"config has a typo at line 10\",\"fix\":\"nginx -t\"}"
                },
                "done": true
            })))
            .mount(&server)
            .await;

        let client: Arc<dyn LlmProvider> = Arc::new(OllamaClient::new(server.uri(), "test".into()));
        let ctx = ShellContext::default();

        let e = explain(&client, "nginx -t", "", "typo at line 10", 1, &ctx)
            .await
            .unwrap();

        assert!(e.text.contains("typo"));
        assert_eq!(e.fix.as_deref(), Some("nginx -t"));
    }
}
