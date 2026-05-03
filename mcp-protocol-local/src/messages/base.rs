// mcp-protocol/src/messages/base.rs
use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Represents a JSON-RPC 2.0 message (request, response, or notification)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    /// A request from client to server or vice versa
    Request {
        jsonrpc: String,
        id: serde_json::Value,
        method: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<serde_json::Value>,
    },
    
    /// A response to a request
    Response {
        jsonrpc: String,
        id: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<JsonRpcError>,
    },
    
    /// A notification (one-way message with no response)
    Notification {
        jsonrpc: String,
        method: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<serde_json::Value>,
    },
}

impl JsonRpcMessage {
    /// Create a new request
    pub fn request(id: serde_json::Value, method: &str, params: Option<serde_json::Value>) -> Self {
        JsonRpcMessage::Request {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        }
    }
    
    /// Create a new response with a result
    pub fn response(id: serde_json::Value, result: serde_json::Value) -> Self {
        JsonRpcMessage::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }
    
    /// Create a new error response
    pub fn error(id: serde_json::Value, code: i32, message: &str, data: Option<serde_json::Value>) -> Self {
        JsonRpcMessage::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data,
            }),
        }
    }
    
    /// Create a new notification
    pub fn notification(method: &str, params: Option<serde_json::Value>) -> Self {
        JsonRpcMessage::Notification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        }
    }
}
