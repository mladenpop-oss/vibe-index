// mcp-protocol/src/types/resource/mod.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a resource that can be accessed by the client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// URI that uniquely identifies the resource
    pub uri: String,
    
    /// Human-readable name of the resource
    pub name: String,
    
    /// Optional description of the resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// Optional MIME type of the resource content
    #[serde(rename = "mimeType")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    
    /// Optional size in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    
    /// Optional custom annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, serde_json::Value>>,
}

/// Content of a resource, which can be either text or binary data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    /// URI that uniquely identifies the resource
    pub uri: String,
    
    /// MIME type of the resource content
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    
    /// Text content (used for text resources)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    
    /// Binary content encoded as base64 (used for binary resources)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

/// Parameters for listing resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesListParams {
    /// Optional cursor for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Result of listing resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesListResult {
    /// List of available resources
    pub resources: Vec<Resource>,
    
    /// Optional cursor for the next page of results
    #[serde(rename = "nextCursor")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Parameters for reading a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceReadParams {
    /// URI of the resource to read
    pub uri: String,
}

/// Result of reading a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceReadResult {
    /// Contents of the resource
    pub contents: Vec<ResourceContent>,
}

/// Parameters for subscribing to a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSubscribeParams {
    /// URI of the resource to subscribe to
    pub uri: String,
}

/// Parameters for a resource update notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUpdatedParams {
    /// URI of the updated resource
    pub uri: String,
}

/// Resource template that can be parameterized
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTemplate {
    /// URI template that can be expanded with parameters
    #[serde(rename = "uriTemplate")]
    pub uri_template: String,
    
    /// Human-readable name of the template
    pub name: String,
    
    /// Optional description of the template
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// Optional MIME type of resources generated from this template
    #[serde(rename = "mimeType")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    
    /// Optional custom annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, serde_json::Value>>,
}

/// Parameters for listing resource templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTemplatesListParams {
    /// Optional cursor for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Result of listing resource templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTemplatesListResult {
    /// List of available resource templates
    #[serde(rename = "resourceTemplates")]
    pub resource_templates: Vec<ResourceTemplate>,
    
    /// Optional cursor for the next page of results
    #[serde(rename = "nextCursor")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Parameters for template parameter completion - DEPRECATED in favor of the general completion API
/// (This is kept for backward compatibility with existing code)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTemplateCompletionParams {
    /// URI template to complete
    #[serde(rename = "uriTemplate")]
    pub uri_template: String,
    
    /// Parameter name to complete
    pub parameter: String,
    
    /// Current value of the parameter (for contextual completion)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Result of template parameter completion - DEPRECATED in favor of the general completion API
/// (This is kept for backward compatibility with existing code)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTemplateCompletionResult {
    /// List of completion suggestions
    pub items: Vec<super::completion::CompletionItem>,
}

/// Parameters for unsubscribing from a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUnsubscribeParams {
    /// URI of the resource to unsubscribe from
    pub uri: String,
}
