use crate::ipc::{Request, RequestHandler, Response, StreamHandler};
use coati_core::{Agent, ChatMessage, Config, OllamaClient, ToolRegistry};
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

    let agent = Arc::new(Agent::new(llm.clone(), registry));

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
                    Request::AskStream { .. } => {
                        // Streaming is dispatched to the stream handler at the transport
                        // layer; this arm is unreachable in practice but returns an error
                        // defensively in case routing changes.
                        Response::Error {
                            message: "ask_stream must be handled by stream dispatcher".into(),
                        }
                    }
                }
            })
        })
    };

    #[cfg(unix)]
    {
        let stream_handler: StreamHandler = {
            let llm = llm.clone();
            let model = cfg.llm.model.clone();
            Arc::new(move |req, wr| {
                let llm = llm.clone();
                let model = model.clone();
                Box::pin(async move {
                    if let Request::AskStream {
                        question,
                        conversation_id,
                    } = req
                    {
                        handle_ask_stream(llm, model, question, conversation_id, wr).await;
                    }
                })
            })
        };

        let transport = crate::ipc::UnixSocketTransport;
        transport
            .serve_with_stream(socket_path, handler, stream_handler)
            .await
    }
    #[cfg(not(unix))]
    {
        let _ = (socket_path, handler);
        anyhow::bail!(
            "no IpcTransport implementation for this platform — IMPLEMENT ME for Windows"
        );
    }
}

#[cfg(unix)]
async fn handle_ask_stream(
    llm: Arc<OllamaClient>,
    model: String,
    question: String,
    conversation_id: Option<String>,
    mut wr: tokio::net::unix::OwnedWriteHalf,
) {
    use tokio::io::AsyncWriteExt;
    use tokio::sync::mpsc;

    // Build message history (prior turns + current user question).
    let mut messages: Vec<ChatMessage> = vec![];
    if let Some(cid) = conversation_id.as_ref() {
        if let Ok(history) = coati_core::history::HistoryRepo::open_default() {
            if let Ok(prior) = history.messages(cid) {
                for m in prior {
                    messages.push(ChatMessage {
                        role: m.role,
                        content: m.content,
                    });
                }
            }
        }
    }
    messages.push(ChatMessage {
        role: "user".into(),
        content: question.clone(),
    });

    // `complete_stream` calls a sync `FnMut(&str)` on every delta and we cannot `.await`
    // inside it. Bridge the sync callback to the async socket writer with an mpsc channel:
    // the callback pushes each delta, a companion task drains the channel and writes Chunk
    // frames to the socket as they arrive. This preserves real-time streaming — frames hit
    // the wire as the model produces them, not all at once at the end.
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let writer_task = tokio::spawn(async move {
        while let Some(delta) = rx.recv().await {
            let frame =
                serde_json::to_string(&Response::Chunk { delta }).unwrap_or_else(|_| "{}".into());
            if wr.write_all(frame.as_bytes()).await.is_err() {
                return Err(wr);
            }
            if wr.write_all(b"\n").await.is_err() {
                return Err(wr);
            }
        }
        Ok(wr)
    });

    let stream_res = llm
        .complete_stream(messages, move |delta| {
            let _ = tx.send(delta.to_string());
        })
        .await;

    // Drop `tx` closes the channel; the writer task will drain remaining deltas and exit.
    let writer_result = writer_task.await;

    let mut wr = match writer_result {
        Ok(Ok(w)) => w,
        // Writer errored mid-stream (client disconnected). Nothing left to do.
        Ok(Err(_)) | Err(_) => return,
    };

    match stream_res {
        Ok(full) => {
            let end_frame = serde_json::to_string(&Response::StreamEnd {
                full_content: full.clone(),
            })
            .unwrap_or_else(|_| "{}".into());
            let _ = wr.write_all(end_frame.as_bytes()).await;
            let _ = wr.write_all(b"\n").await;
            let _ = wr.flush().await;

            if let Some(cid) = conversation_id.as_ref() {
                if let Ok(history) = coati_core::history::HistoryRepo::open_default() {
                    let _ = history.append_message(cid, "user", &question, &model);
                    let _ = history.append_message(cid, "assistant", &full, &model);
                }
            }
        }
        Err(e) => {
            let err_frame = serde_json::to_string(&Response::Error {
                message: e.to_string(),
            })
            .unwrap_or_else(|_| "{}".into());
            let _ = wr.write_all(err_frame.as_bytes()).await;
            let _ = wr.write_all(b"\n").await;
            let _ = wr.flush().await;
        }
    }
}
