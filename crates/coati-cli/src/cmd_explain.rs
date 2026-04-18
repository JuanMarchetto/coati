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
