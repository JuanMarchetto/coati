# coati-mcp-server

An **MCP (Model Context Protocol) server** that exposes Coati's typed Linux
system tools to any MCP client — [goose](https://github.com/aaif-goose/goose),
Claude Desktop, Cursor, or your own agent.

It is a thin adapter: it reuses Coati's existing `ToolRegistry` and the JSON
Schemas that `schemars` already derives for each tool, then serves them over the
standard MCP `stdio` transport (newline-delimited JSON-RPC 2.0). No protocol SDK
is pulled in — the handshake and the three methods a tool server needs
(`initialize`, `tools/list`, `tools/call`, plus `ping`) are implemented directly,
keeping the dependency footprint to `tokio` + `serde_json`.

## Tools exposed

| Tool | What it does | Notes |
|------|--------------|-------|
| `exec` | Run a program (arguments passed literally — **no shell**, no piping/redirection) | Omitted in `--read-only` mode |
| `read_file` | Read a file up to `max_bytes` (default 64 KiB) | Read-only |
| `list_dir` | List a directory (non-recursive) | Read-only |
| `query_logs` | Fetch recent `journalctl` lines for a systemd unit | Linux only; unit name is allowlist-validated |
| `explain_error` | Package a failed command's stdout/stderr/exit code into an analysis prompt | Pure text |

## Safety

Coati's design is "the agent *proposes*, a human *confirms*." This server keeps
that contract: it executes only what the MCP client asks, and **the client (e.g.
goose) is responsible for getting user confirmation before a tool runs.** `exec`
never invokes a shell, so there is no shell-injection surface, and `query_logs`
validates unit names against `[a-zA-Z0-9@._-]+` before calling `journalctl`. Run
with `--read-only` to drop `exec` entirely and expose inspection tools only.

## Install

```bash
cargo install --git https://github.com/JuanMarchetto/coati --bin coati-mcp
```

## Use with goose

Add it as a command-line (stdio) extension:

```bash
goose configure        # → Add Extension → Command-line Extension
# Command: coati-mcp
```

Or in goose's config (`~/.config/goose/config.yaml`):

```yaml
extensions:
  coati:
    type: stdio
    cmd: coati-mcp
    args: []          # add "--read-only" to disable the exec tool
    enabled: true
```

## Run / debug manually

```bash
# Read-only inspection tools only
coati-mcp --read-only

# Drive it by hand (one JSON-RPC message per line):
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}' \
              '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | coati-mcp
```

## License

Apache-2.0, same as the rest of Coati.
