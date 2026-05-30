//! Minimal MCP (Model Context Protocol) server over stdio for Coati's tools.
//!
//! Speaks JSON-RPC 2.0 framed as newline-delimited JSON, the standard MCP
//! `stdio` transport. Tool metadata (name, description, JSON Schema) is taken
//! straight from Coati's [`ToolRegistry`], so the MCP surface stays in sync with
//! the tools Coati already exposes to its own agent. No protocol SDK dependency:
//! the handshake and the three methods a tool server needs are implemented here.

use coati_core::ToolRegistry;
use serde_json::{json, Value};

/// MCP protocol revision used when the client does not request a specific one.
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-06-18";
/// Server name reported during `initialize`.
pub const SERVER_NAME: &str = "coati-mcp";

struct ToolDef {
    name: String,
    description: String,
    input_schema: Value,
}

/// An MCP server backed by a Coati [`ToolRegistry`].
pub struct McpServer {
    registry: ToolRegistry,
    tools: Vec<ToolDef>,
}

/// A JSON-RPC protocol error (distinct from a tool-execution error, which is
/// returned inside a successful `tools/call` result with `isError: true`).
struct RpcError {
    code: i64,
    message: String,
}

impl RpcError {
    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("method not found: {method}"),
        }
    }

    fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
        }
    }
}

impl McpServer {
    /// Build a server from a populated registry, snapshotting its tool metadata.
    pub fn new(registry: ToolRegistry) -> Self {
        let tools = registry
            .descriptions()
            .into_iter()
            .map(|(name, description, input_schema)| ToolDef {
                name: name.to_string(),
                description: description.to_string(),
                input_schema,
            })
            .collect();
        Self { registry, tools }
    }

    /// Handle one parsed JSON-RPC message.
    ///
    /// Returns `Some(response)` for requests (messages with a non-null `id`) and
    /// `None` for notifications and anything lacking a `method` (e.g. responses).
    pub async fn handle(&self, message: Value) -> Option<Value> {
        let method = message.get("method").and_then(Value::as_str)?;

        // Notifications carry a method but no id; act on them, never reply.
        let id = match message.get("id").cloned() {
            Some(id) if !id.is_null() => id,
            _ => return None,
        };

        let outcome = match method {
            "initialize" => Ok(self.initialize_result(message.get("params"))),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(self.tools_list_result()),
            "tools/call" => self.tools_call(message.get("params")).await,
            other => Err(RpcError::method_not_found(other)),
        };

        Some(match outcome {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(err) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": err.code, "message": err.message },
            }),
        })
    }

    fn initialize_result(&self, params: Option<&Value>) -> Value {
        // Echo the client's requested protocol version when present, for
        // maximum compatibility; otherwise advertise the one we default to.
        let protocol_version = params
            .and_then(|p| p.get("protocolVersion"))
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_PROTOCOL_VERSION);

        json!({
            "protocolVersion": protocol_version,
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": env!("CARGO_PKG_VERSION"),
            },
            "instructions": "Coati exposes safe, typed Linux system tools: run a program \
                (no shell interpretation, no piping), read a file, list a directory, query \
                systemd service logs, and package a failed command's output for diagnosis. \
                The calling client is responsible for confirming actions with the user before \
                running them.",
        })
    }

    fn tools_list_result(&self) -> Value {
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": tool.input_schema,
                })
            })
            .collect();
        json!({ "tools": tools })
    }

    async fn tools_call(&self, params: Option<&Value>) -> Result<Value, RpcError> {
        let params = params.ok_or_else(|| RpcError::invalid_params("missing params"))?;
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError::invalid_params("missing tool name"))?;
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        // Tool-execution failures (unknown tool, bad input, runtime error) are
        // reported as a successful result with isError=true, per MCP, so the
        // model can read and react to the error text.
        match self.registry.call(name, arguments).await {
            Ok(value) => Ok(json!({
                "content": [{ "type": "text", "text": value.to_string() }],
                "isError": false,
            })),
            Err(err) => Ok(json!({
                "content": [{ "type": "text", "text": err.to_string() }],
                "isError": true,
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::build_registry;

    fn server(read_only: bool) -> McpServer {
        McpServer::new(build_registry(read_only))
    }

    #[tokio::test]
    async fn initialize_reports_server_info_and_echoes_protocol() {
        let resp = server(false)
            .handle(json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-03-26" }
            }))
            .await
            .expect("initialize must reply");

        let result = &resp["result"];
        assert_eq!(result["protocolVersion"], "2025-03-26");
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[tokio::test]
    async fn tools_list_includes_core_tools_with_object_schemas() {
        let resp = server(false)
            .handle(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }))
            .await
            .expect("tools/list must reply");

        let tools = resp["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for expected in ["exec", "read_file", "list_dir", "explain_error"] {
            assert!(names.contains(&expected), "missing tool {expected}");
        }
        for tool in tools {
            assert_eq!(
                tool["inputSchema"]["type"], "object",
                "tool {} should advertise an object input schema",
                tool["name"]
            );
        }
    }

    #[tokio::test]
    async fn read_only_hides_exec_but_keeps_read_file() {
        let resp = server(true)
            .handle(json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" }))
            .await
            .unwrap();

        let names: Vec<String> = resp["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert!(!names.contains(&"exec".to_string()), "exec must be hidden");
        assert!(names.contains(&"read_file".to_string()));
    }

    #[tokio::test]
    async fn tools_call_runs_exec_and_returns_output() {
        let resp = server(false)
            .handle(json!({
                "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": {
                    "name": "exec",
                    "arguments": { "command": "echo", "args": ["coati-hi"] }
                }
            }))
            .await
            .unwrap();

        assert_eq!(resp["result"]["isError"], false);
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("coati-hi"), "stdout missing in: {text}");
    }

    #[tokio::test]
    async fn unknown_tool_is_reported_as_tool_error() {
        let resp = server(false)
            .handle(json!({
                "jsonrpc": "2.0", "id": 5, "method": "tools/call",
                "params": { "name": "does_not_exist", "arguments": {} }
            }))
            .await
            .unwrap();

        assert_eq!(resp["result"]["isError"], true);
    }

    #[tokio::test]
    async fn missing_tool_name_is_invalid_params() {
        let resp = server(false)
            .handle(json!({
                "jsonrpc": "2.0", "id": 6, "method": "tools/call",
                "params": { "arguments": {} }
            }))
            .await
            .unwrap();

        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn unknown_method_is_method_not_found() {
        let resp = server(false)
            .handle(json!({ "jsonrpc": "2.0", "id": 7, "method": "frobnicate" }))
            .await
            .unwrap();

        assert_eq!(resp["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn notifications_get_no_reply() {
        let resp = server(false)
            .handle(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .await;

        assert!(resp.is_none());
    }
}
