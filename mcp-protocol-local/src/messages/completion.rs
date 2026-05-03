use serde::{Deserialize, Serialize};
use crate::types::completion::{CompleteRequest, CompleteResponse};

/// Message to request completion suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionCompleteRequest {
    #[serde(flatten)]
    pub params: CompleteRequest,
}

/// Response for completion suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionCompleteResponse {
    #[serde(flatten)]
    pub result: CompleteResponse,
}
