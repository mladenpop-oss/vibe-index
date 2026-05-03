// mcp-protocol/src/types/sampling/mod.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Sampling message content types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageContent {
    #[serde(rename = "text")]
    Text {
        text: String,
    },
    #[serde(rename = "image")]
    Image {
        data: String,
        mime_type: String,
    },
}

/// Message role in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

/// Model hint for sampling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelHint {
    pub name: String,
}

/// Model preferences for sampling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPreferences {
    /// Hints for specific models or model families
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<Vec<ModelHint>>,
    
    /// Priority for cost (0.0-1.0), higher values prefer cheaper models
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_priority: Option<f32>,
    
    /// Priority for speed (0.0-1.0), higher values prefer faster models
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_priority: Option<f32>,
    
    /// Priority for intelligence (0.0-1.0), higher values prefer more capable models
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intelligence_priority: Option<f32>,
}

/// Params for creating a sampling message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageParams {
    /// The conversation messages to include
    pub messages: Vec<Message>,
    
    /// Model preferences for selection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_preferences: Option<ModelPreferences>,
    
    /// Optional system prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    
    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    
    /// Optional temperature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    
    /// Optional top_p value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    
    /// Optional sampling context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<HashMap<String, String>>,
}

/// Response for a sampling message creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageResult {
    /// The role of the response message
    pub role: String,
    
    /// The content of the response
    pub content: MessageContent,
    
    /// The model used for generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    
    /// The reason why generation stopped
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}
