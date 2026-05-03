// mcp-protocol/src/messages/lifecycle.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::{ClientInfo, ServerInfo};

/// Client capabilities negotiated during initialization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<HashMap<String, bool>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<HashMap<String, serde_json::Value>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
}

/// Server capabilities negotiated during initialization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<HashMap<String, serde_json::Value>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<HashMap<String, bool>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<HashMap<String, bool>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<HashMap<String, bool>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
}

/// Parameters for the initialize request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    
    pub capabilities: ClientCapabilities,
    
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
}

/// Result of the initialize request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    
    pub capabilities: ServerCapabilities,
    
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}
