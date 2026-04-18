pub mod ipc;
pub use ipc::{Request, Response, ShellContext};

pub mod tool;
pub use tool::{Tool, ToolError, ToolRegistry};

pub mod agent;
pub use agent::Agent;

pub mod system;
pub use system::{is_valid_unit_name, SystemLogError, SystemLogProvider};

pub mod llm;
pub use llm::{ChatMessage, LlmProvider, LlmResponse, LlmToolCall, OllamaClient};

pub mod config;
pub use config::{Config, LlmConfig, ToolsConfig};

pub mod agent_ext;
pub use agent_ext::{explain, propose, Explanation, Proposal};

pub mod history;
pub use history::{Conversation, HistoryRepo, Message};

#[cfg(target_os = "linux")]
pub use system::LinuxJournalLogProvider;
