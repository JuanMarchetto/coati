//! `coati-mcp` — exposes Coati's typed system tools to any MCP client (e.g. goose)
//! over the standard Model Context Protocol `stdio` transport.
//!
//! Messages are newline-delimited JSON-RPC 2.0 objects on stdin/stdout. Run with
//! `--read-only` to omit the `exec` tool and expose only inspection tools.

mod registry;
mod server;

use server::McpServer;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let read_only = std::env::args().skip(1).any(|arg| arg == "--read-only");
    let server = McpServer::new(registry::build_registry(read_only));

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(message) => server.handle(message).await,
            Err(err) => Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": serde_json::Value::Null,
                "error": { "code": -32700, "message": format!("parse error: {err}") },
            })),
        };

        if let Some(response) = response {
            let mut bytes = serde_json::to_vec(&response)?;
            bytes.push(b'\n');
            stdout.write_all(&bytes).await?;
            stdout.flush().await?;
        }
    }

    Ok(())
}
