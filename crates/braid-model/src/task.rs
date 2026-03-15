use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TaskContext {
    pub task_id: Option<String>,
    pub summary: String,
}
