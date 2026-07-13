pub mod engine;
pub mod planner;
pub mod registry;
pub mod tools;

// Re-export port traits at crate root for backward compatibility
pub use braid_ports::{EventSink, Provider, Redactor, ToolExecutor};
pub use engine::{Engine, RunInput, RunOutput};
pub use planner::{Action, Planner, SessionState, SimpleLoopPlanner};
pub use registry::ToolRegistry;
pub use tools::StaticTool;
