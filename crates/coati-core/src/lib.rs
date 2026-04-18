pub mod tool;
pub use tool::{Tool, ToolError, ToolRegistry};

pub mod system;
pub use system::{SystemLogProvider, SystemLogError, is_valid_unit_name};

#[cfg(target_os = "linux")]
pub use system::LinuxJournalLogProvider;
