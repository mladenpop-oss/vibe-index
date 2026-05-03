// mcp-protocol/src/types/server.rs
use serde::{Deserialize, Serialize};

/// Information about the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Enum representing server state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    Created,
    Initializing,
    Ready,
    ShuttingDown,
}
