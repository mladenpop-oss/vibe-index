// mcp-protocol/src/types/tool.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Definition of a tool that can be called by the client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[cfg_attr(feature = "camel_case", serde(rename = "inputSchema"))]
    pub input_schema: serde_json::Value,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, serde_json::Value>>,
}

/// Parameters for a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallParams {
    pub name: String,

    pub arguments: serde_json::Value,
}

/// A single content item in a tool result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },

    #[serde(rename = "audio")]
    Audio {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },

    #[serde(rename = "resource")]
    Resource { resource: serde_json::Value },
}

/// Result of a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,

    #[serde(rename = "isError")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Parameters for listing tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Result of listing tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsListResult {
    pub tools: Vec<Tool>,

    #[serde(rename = "nextCursor")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}
