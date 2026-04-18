use crate::ipc::{IpcTransport, Request, RequestHandler, Response};
use coati_core::{Agent, Config, OllamaClient, ToolRegistry};
use coati_tools::{ExecTool, ExplainErrorTool, ListDirTool, QueryLogsTool, ReadFileTool};
use std::sync::Arc;

pub async fn run(socket_path: &str) -> anyhow::Result<()> {
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

    let agent = Arc::new(Agent::new(llm, registry));

    let handler: RequestHandler = {
        let agent = agent.clone();
        Arc::new(move |req: Request| {
            let agent = agent.clone();
            Box::pin(async move {
                match req {
                    Request::Ping => Response::Pong,
                    Request::Ask { question } => match agent.respond(&question).await {
                        Ok(content) => Response::Answer { content },
                        Err(e) => Response::Error {
                            message: e.to_string(),
                        },
                    },
                    Request::Propose { .. } | Request::Explain { .. } => Response::Error {
                        message: "not implemented yet".into(),
                    },
                }
            })
        })
    };

    #[cfg(unix)]
    {
        let transport = crate::ipc::UnixSocketTransport;
        transport.serve(socket_path, handler).await
    }
    #[cfg(not(unix))]
    {
        let _ = (socket_path, handler);
        anyhow::bail!(
            "no IpcTransport implementation for this platform — IMPLEMENT ME for Windows"
        );
    }
}
