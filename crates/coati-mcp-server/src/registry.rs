//! Builds the [`ToolRegistry`] that the MCP server exposes.

use coati_core::ToolRegistry;
use coati_tools::{ExecTool, ExplainErrorTool, ListDirTool, ReadFileTool};

/// Build the registry of tools to expose over MCP.
///
/// When `read_only` is `true` the `exec` tool (which runs programs) is omitted,
/// leaving only the inspection tools. The systemd log tool is registered only
/// on Linux, where a journal provider is available.
pub fn build_registry(read_only: bool) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    registry.register(ReadFileTool);
    registry.register(ListDirTool);
    registry.register(ExplainErrorTool);

    if !read_only {
        registry.register(ExecTool::default());
    }

    #[cfg(target_os = "linux")]
    {
        use coati_core::LinuxJournalLogProvider;
        use coati_tools::QueryLogsTool;
        use std::sync::Arc;

        registry.register(QueryLogsTool::new(Arc::new(LinuxJournalLogProvider)));
    }

    registry
}
