// mcp-protocol/src/lib.rs
pub mod constants;
pub mod messages;
pub mod types;
pub mod version;

// Re-export commonly used items
pub use constants::PROTOCOL_VERSION;
pub use messages::JsonRpcMessage;
pub use types::*;
