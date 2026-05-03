// mcp-protocol/src/types/prompt.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Prompt definition provided by the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    /// Unique identifier for the prompt
    pub name: String,
    
    /// Optional human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// Optional list of arguments for customization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
    
    /// Optional annotations for additional information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, serde_json::Value>>,
}

/// Argument definition for a prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    /// Argument name
    pub name: String,
    
    /// Optional human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// Whether the argument is required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// Parameters for the prompts/list request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsListParams {
    /// Optional cursor for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Result of the prompts/list request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsListResult {
    /// List of available prompts
    pub prompts: Vec<Prompt>,
    
    /// Cursor for the next page (empty if no more pages)
    #[serde(rename = "nextCursor")]
    pub next_cursor: String,
}

/// Parameters for the prompts/get request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptGetParams {
    /// Name of the prompt to retrieve
    pub name: String,
    
    /// Arguments to apply to the prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<HashMap<String, String>>,
}

/// Result of the prompts/get request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptGetResult {
    /// Optional human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// Messages in the prompt
    pub messages: Vec<PromptMessage>,
}

/// Message in a prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    /// Role of the message sender ("user" or "assistant")
    pub role: String,
    
    /// Content of the message
    pub content: PromptMessageContent,
}

/// Content of a prompt message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PromptMessageContent {
    /// Text content
    #[serde(rename = "text")]
    Text { text: String },
    
    /// Image content
    #[serde(rename = "image")]
    Image { 
        data: String, 
        #[serde(rename = "mimeType")]
        mime_type: String 
    },
    
    /// Resource content
    #[serde(rename = "resource")]
    Resource { 
        resource: EmbeddedResource 
    },
}

/// Embedded resource in a prompt message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedResource {
    /// URI of the resource
    pub uri: String,
    
    /// MIME type of the resource
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    
    /// Text content (if text resource)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    
    /// Binary data content (if binary resource)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

/// Reference to a prompt for completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptReference {
    /// Type of reference (always "ref/prompt")
    #[serde(rename = "type")]
    pub ref_type: String,
    
    /// Name of the prompt
    pub name: String,
}
