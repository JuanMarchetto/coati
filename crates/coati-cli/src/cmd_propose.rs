use coati_core::ipc::ShellContext;
use coati_core::{propose, Config, OllamaClient};
use std::sync::Arc;

pub async fn run(intent: &str, json: bool, context_override: Option<&str>) -> anyhow::Result<()> {
    let cfg = Config::load_or_default()?;
    let llm: Arc<dyn coati_core::LlmProvider> = Arc::new(OllamaClient::new(
        cfg.llm.endpoint.clone(),
        cfg.llm.model.clone(),
    ));

    let ctx = if let Some(raw) = context_override {
        serde_json::from_str::<ShellContext>(raw)
            .map_err(|e| anyhow::anyhow!("invalid --context JSON: {e}"))?
    } else {
        auto_context()
    };

    let p = propose(&llm, intent, &ctx).await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "command": p.command,
                "reasoning": p.reasoning,
                "needs_sudo": p.needs_sudo,
            })
        );
    } else {
        if p.needs_sudo {
            println!("⚠ needs sudo");
        }
        println!("$ {}", p.command);
        println!("  → {}", p.reasoning);
    }
    Ok(())
}

fn auto_context() -> ShellContext {
    ShellContext {
        pwd: std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(String::from))
            .unwrap_or_default(),
        shell: std::env::var("SHELL")
            .ok()
            .and_then(|s| s.rsplit('/').next().map(String::from))
            .unwrap_or_default(),
        ..Default::default()
    }
}
