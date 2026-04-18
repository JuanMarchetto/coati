use async_trait::async_trait;
use futures::future::BoxFuture;
use std::sync::Arc;

#[allow(unused_imports)]
pub use coati_core::ipc::{Request, Response, ShellContext};

/// Handles an incoming Request, returns a Response. Shared handler used by all transports.
pub type RequestHandler = Arc<dyn Fn(Request) -> BoxFuture<'static, Response> + Send + Sync>;

/// Handles a streaming Request by writing many response frames directly to the connection.
/// The handler is responsible for writing a terminal frame (`StreamEnd` or `Error`) before
/// returning; the transport will close the connection after the handler returns.
#[cfg(unix)]
pub type StreamHandler =
    Arc<dyn Fn(Request, tokio::net::unix::OwnedWriteHalf) -> BoxFuture<'static, ()> + Send + Sync>;

/// Abstract transport layer so platforms can differ (Unix socket / Windows named pipe / TCP).
#[async_trait]
pub trait IpcTransport: Send + Sync {
    /// Bind the transport at the given address, start accepting connections, dispatch each
    /// request through `handler`. Runs until cancelled.
    #[allow(dead_code)]
    async fn serve(&self, address: &str, handler: RequestHandler) -> anyhow::Result<()>;
}

#[cfg(unix)]
pub struct UnixSocketTransport;

#[cfg(unix)]
impl UnixSocketTransport {
    /// Like `serve`, but also routes `Request::AskStream` frames to a streaming handler that
    /// owns the write half of the connection and emits multiple `Response::Chunk` frames
    /// followed by a terminal `Response::StreamEnd` or `Response::Error`.
    pub async fn serve_with_stream(
        &self,
        address: &str,
        handler: RequestHandler,
        stream_handler: StreamHandler,
    ) -> anyhow::Result<()> {
        use std::path::Path;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixListener;

        let expanded = shellexpand::tilde(address).into_owned();
        let path = Path::new(&expanded);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::remove_file(path);

        let listener = UnixListener::bind(path)?;
        tracing::info!(socket = %path.display(), "coati daemon ready");

        loop {
            let (stream, _) = listener.accept().await?;
            let handler = handler.clone();
            let stream_handler = stream_handler.clone();
            tokio::spawn(async move {
                let (rd, mut wr) = stream.into_split();
                let mut reader = BufReader::new(rd);
                let mut line = String::new();
                loop {
                    line.clear();
                    let n = match reader.read_line(&mut line).await {
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    if n == 0 {
                        break;
                    }
                    match serde_json::from_str::<Request>(line.trim()) {
                        Ok(req @ Request::AskStream { .. }) => {
                            // Hand the write half to the streaming handler; it will emit
                            // multiple frames and return. The connection is consumed.
                            stream_handler(req, wr).await;
                            return;
                        }
                        Ok(req) => {
                            let resp = handler(req).await;
                            let body = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".into());
                            if wr.write_all(body.as_bytes()).await.is_err() {
                                break;
                            }
                            if wr.write_all(b"\n").await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let resp = Response::Error {
                                message: format!("bad request: {e}"),
                            };
                            let body = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".into());
                            if wr.write_all(body.as_bytes()).await.is_err() {
                                break;
                            }
                            if wr.write_all(b"\n").await.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
    }
}

#[cfg(unix)]
#[async_trait]
impl IpcTransport for UnixSocketTransport {
    async fn serve(&self, address: &str, handler: RequestHandler) -> anyhow::Result<()> {
        use std::path::Path;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixListener;

        let expanded = shellexpand::tilde(address).into_owned();
        let path = Path::new(&expanded);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::remove_file(path);

        let listener = UnixListener::bind(path)?;
        tracing::info!(socket = %path.display(), "coati daemon ready");

        loop {
            let (stream, _) = listener.accept().await?;
            let handler = handler.clone();
            tokio::spawn(async move {
                let (rd, mut wr) = stream.into_split();
                let mut reader = BufReader::new(rd);
                let mut line = String::new();
                loop {
                    line.clear();
                    let n = match reader.read_line(&mut line).await {
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    if n == 0 {
                        break;
                    }
                    let resp = match serde_json::from_str::<Request>(line.trim()) {
                        Ok(req) => handler(req).await,
                        Err(e) => Response::Error {
                            message: format!("bad request: {e}"),
                        },
                    };
                    let body = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".into());
                    if wr.write_all(body.as_bytes()).await.is_err() {
                        break;
                    }
                    if wr.write_all(b"\n").await.is_err() {
                        break;
                    }
                }
            });
        }
    }
}
