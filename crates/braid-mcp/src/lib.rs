#![cfg_attr(test, allow(clippy::unwrap_used))]
pub mod server;
pub mod tools;

pub use server::run_mcp_server;
pub use tools::McpToolRegistry;
pub use tools::echo::echo_tool;
