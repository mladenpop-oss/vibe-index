// mcp-protocol/src/constants.rs

/// The current protocol version
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// All supported protocol versions (for backward compatibility)
pub const SUPPORTED_VERSIONS: &[&str] = &["2025-06-18", "2024-11-05"];

/// JSON-RPC method names
pub mod methods {
    // Lifecycle methods
    pub const INITIALIZE: &str = "initialize";
    pub const INITIALIZED: &str = "notifications/initialized";

    // Tool methods
    pub const TOOLS_LIST: &str = "tools/list";
    pub const TOOLS_CALL: &str = "tools/call";

    // Tool notifications
    pub const TOOLS_LIST_CHANGED: &str = "notifications/tools/list_changed";

    // Resource methods
    pub const RESOURCES_LIST: &str = "resources/list";
    pub const RESOURCES_READ: &str = "resources/read";
    pub const RESOURCES_SUBSCRIBE: &str = "resources/subscribe";
    pub const RESOURCES_UNSUBSCRIBE: &str = "resources/unsubscribe";

    // Resource template methods
    pub const RESOURCES_TEMPLATES_LIST: &str = "resources/templates/list";

    // Prompt methods
    pub const PROMPTS_LIST: &str = "prompts/list";
    pub const PROMPTS_GET: &str = "prompts/get";

    // Prompt notifications
    pub const PROMPTS_LIST_CHANGED: &str = "notifications/prompts/list_changed";

    // Completion methods
    pub const COMPLETION_COMPLETE: &str = "completion/complete";

    // Resource notifications
    pub const RESOURCES_UPDATED: &str = "notifications/resources/updated";
    pub const RESOURCES_LIST_CHANGED: &str = "notifications/resources/list_changed";

    // Sampling methods
    pub const SAMPLING_CREATE_MESSAGE: &str = "sampling/createMessage";

    // Logging notifications
    pub const LOG: &str = "notifications/log";
}

/// JSON-RPC error codes
pub mod error_codes {
    // Standard JSON-RPC error codes
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // MCP specific error codes
    pub const RESOURCE_NOT_FOUND: i32 = -32002;
    pub const SERVER_NOT_INITIALIZED: i32 = -32003;
    pub const SAMPLING_NOT_ENABLED: i32 = -32004;
    pub const SAMPLING_NO_CALLBACK: i32 = -32005;
    pub const SAMPLING_ERROR: i32 = -32006;
}
