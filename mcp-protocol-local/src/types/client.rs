// mcp-protocol/src/types/client.rs
use serde::{Deserialize, Serialize};

/// Information about the client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}
