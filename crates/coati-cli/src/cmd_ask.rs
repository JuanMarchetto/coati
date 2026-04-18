use coati_core::{Agent, Config, OllamaClient, ToolRegistry};
use coati_tools::{ExecTool, ExplainErrorTool, ListDirTool, QueryLogsTool, ReadFileTool};
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
    let llm = Arc::new(OllamaClient::new(
        cfg.llm.endpoint.clone(),
        cfg.llm.model.clone(),
    ));

    let mut registry = ToolRegistry::new();
    let enabled: std::collections::HashSet<&str> =
        cfg.tools.enabled.iter().map(|s| s.as_str()).collect();
    if enabled.contains("exec") {
        registry.register(ExecTool::default());
    }
    if enabled.contains("read_file") {
        registry.register(ReadFileTool);
    }
    if enabled.contains("list_dir") {
        registry.register(ListDirTool);
    }
    if enabled.contains("query_logs") {
        #[cfg(target_os = "linux")]
        {
            let provider: Arc<dyn coati_core::SystemLogProvider> =
                Arc::new(coati_core::LinuxJournalLogProvider);
            registry.register(QueryLogsTool::new(provider));
        }
    }
    if enabled.contains("explain_error") {
        registry.register(ExplainErrorTool);
    }

    let agent = Agent::new(llm, registry);
    let reply = agent.respond(&q).await?;
    println!("{}", reply);
    Ok(())
}
