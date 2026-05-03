use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use vibe_index::VibeIndex;

pub struct VibeMcpServer {
    index: Arc<Mutex<VibeIndex>>,
}

impl VibeMcpServer {
    pub fn new() -> Self {
        Self {
            index: Arc::new(Mutex::new(VibeIndex::new())),
        }
    }

    fn index_text_handler(
        index: Arc<Mutex<VibeIndex>>,
        args: serde_json::Value,
    ) -> anyhow::Result<modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult>
    {
        let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let tokens: Vec<String> = text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        let index_clone = Arc::clone(&index);
        let result = tokio::task::block_in_place(|| {
            let mut idx = index_clone.blocking_lock();
            for token in &tokens {
                idx.add_token(token);
            }
            (tokens.len(), idx.unique_tokens(), idx.total_positions())
        });

        Ok(
            modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult {
                content: vec![
                    modelcontextprotocol_server::mcp_protocol::types::tool::ToolContent::Text {
                        text: format!(
                            "Indexed {} tokens ({} unique). Total positions: {}",
                            result.0, result.1, result.2
                        ),
                    },
                ],
                is_error: None,
            },
        )
    }

    fn phrase_search_handler(
        index: Arc<Mutex<VibeIndex>>,
        args: serde_json::Value,
    ) -> anyhow::Result<modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult>
    {
        let phrase = args.get("phrase").and_then(|v| v.as_str()).unwrap_or("");
        let tokens: Vec<String> = phrase
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        let index_clone = Arc::clone(&index);
        let results = tokio::task::block_in_place(|| {
            let idx = index_clone.blocking_lock();
            idx.phrase_search(&tokens)
        });

        let output: String = if results.is_empty() {
            "No matches found.".to_string()
        } else {
            results
                .iter()
                .map(|r| {
                    format!(
                        "[POS {}] conf={:.2} {}",
                        r.position, r.confidence, r.context
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(
            modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult {
                content: vec![
                    modelcontextprotocol_server::mcp_protocol::types::tool::ToolContent::Text {
                        text: output,
                    },
                ],
                is_error: None,
            },
        )
    }

    fn fuzzy_search_handler(
        index: Arc<Mutex<VibeIndex>>,
        args: serde_json::Value,
    ) -> anyhow::Result<modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult>
    {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let max_distance = args
            .get("max_distance")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;

        let index_clone = Arc::clone(&index);
        let results = tokio::task::block_in_place(|| {
            let idx = index_clone.blocking_lock();
            idx.fuzzy_search(query, max_distance)
        });

        let output: String = if results.is_empty() {
            "No fuzzy matches found.".to_string()
        } else {
            results
                .iter()
                .map(|r| {
                    format!(
                        "[POS {}] conf={:.2} {}",
                        r.position, r.confidence, r.context
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(
            modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult {
                content: vec![
                    modelcontextprotocol_server::mcp_protocol::types::tool::ToolContent::Text {
                        text: output,
                    },
                ],
                is_error: None,
            },
        )
    }

    fn search_handler(
        index: Arc<Mutex<VibeIndex>>,
        args: serde_json::Value,
    ) -> anyhow::Result<modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult>
    {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");

        let index_clone = Arc::clone(&index);
        let results = tokio::task::block_in_place(|| {
            let idx = index_clone.blocking_lock();
            idx.search(query)
        });

        let output: String = if results.is_empty() {
            "No matches found.".to_string()
        } else {
            results
                .iter()
                .map(|r| {
                    format!(
                        "[POS {}] conf={:.2} {}",
                        r.position, r.confidence, r.context
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(
            modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult {
                content: vec![
                    modelcontextprotocol_server::mcp_protocol::types::tool::ToolContent::Text {
                        text: output,
                    },
                ],
                is_error: None,
            },
        )
    }

    fn get_stats_handler(
        index: Arc<Mutex<VibeIndex>>,
        _args: serde_json::Value,
    ) -> anyhow::Result<modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult>
    {
        let index_clone = Arc::clone(&index);
        let (positions, unique, mem_bytes) = tokio::task::block_in_place(|| {
            let idx = index_clone.blocking_lock();
            (
                idx.total_positions(),
                idx.unique_tokens(),
                idx.estimated_memory_bytes(),
            )
        });

        let mem_kb = mem_bytes as f64 / 1024.0;
        let output = format!(
            "Total positions: {}\nUnique tokens: {}\nMemory: {:.2} KB ({} bytes)",
            positions, unique, mem_kb, mem_bytes
        );

        Ok(
            modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult {
                content: vec![
                    modelcontextprotocol_server::mcp_protocol::types::tool::ToolContent::Text {
                        text: output,
                    },
                ],
                is_error: None,
            },
        )
    }

    fn clear_index_handler(
        index: Arc<Mutex<VibeIndex>>,
        _args: serde_json::Value,
    ) -> anyhow::Result<modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult>
    {
        let index_clone = Arc::clone(&index);
        tokio::task::block_in_place(|| {
            let mut idx = index_clone.blocking_lock();
            *idx = VibeIndex::new();
        });

        Ok(
            modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult {
                content: vec![
                    modelcontextprotocol_server::mcp_protocol::types::tool::ToolContent::Text {
                        text: "Index cleared.".to_string(),
                    },
                ],
                is_error: None,
            },
        )
    }

    pub fn tools(&self) -> Vec<ToolDef> {
        vec![
            {
                let idx = Arc::clone(&self.index);
                ToolDef {
                    name: "index_text".to_string(),
                    description: Some("Index a text document into the Vibe Index. Splits text into tokens and adds them to the index. Returns token count and unique token count.".to_string()),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "text": {
                                "type": "string",
                                "description": "The text content to index"
                            }
                        },
                        "required": ["text"]
                    }),
                    handler: Box::new(move |args| Self::index_text_handler(idx.clone(), args)),
                }
            },
            {
                let idx = Arc::clone(&self.index);
                ToolDef {
                    name: "phrase_search".to_string(),
                    description: Some("Search for an exact phrase in the indexed text. Provides exact token positions. Returns matches with position, confidence, and context.".to_string()),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "phrase": {
                                "type": "string",
                                "description": "The phrase to search for (space-separated tokens)"
                            }
                        },
                        "required": ["phrase"]
                    }),
                    handler: Box::new(move |args| Self::phrase_search_handler(idx.clone(), args)),
                }
            },
            {
                let idx = Arc::clone(&self.index);
                ToolDef {
                    name: "fuzzy_search".to_string(),
                    description: Some("Search with typo tolerance. Finds tokens that are similar to the query using Levenshtein distance. Uses bigram prefiltering for speed (97% fewer computations).".to_string()),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "The query term (may contain typos)"
                            },
                            "max_distance": {
                                "type": "integer",
                                "description": "Maximum Levenshtein distance (1 = 1 typo, 2 = 2 typos). Default: 1"
                            }
                        },
                        "required": ["query"]
                    }),
                    handler: Box::new(move |args| Self::fuzzy_search_handler(idx.clone(), args)),
                }
            },
            {
                let idx = Arc::clone(&self.index);
                ToolDef {
                    name: "search".to_string(),
                    description: Some("Unified natural language search. Parses NL query into search phrases, runs phrase search + fuzzy search, merges and ranks results by confidence. Best for natural language queries like 'where is the authenticate function'.".to_string()),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Natural language search query"
                            }
                        },
                        "required": ["query"]
                    }),
                    handler: Box::new(move |args| Self::search_handler(idx.clone(), args)),
                }
            },
            {
                let idx = Arc::clone(&self.index);
                ToolDef {
                    name: "get_stats".to_string(),
                    description: Some("Get index statistics: total positions, unique tokens, estimated memory usage.".to_string()),
                    input_schema: json!({
                        "type": "object",
                        "properties": {}
                    }),
                    handler: Box::new(move |args| Self::get_stats_handler(idx.clone(), args)),
                }
            },
            {
                let idx = Arc::clone(&self.index);
                ToolDef {
                    name: "clear_index".to_string(),
                    description: Some(
                        "Clear all indexed data and reset the index to empty state.".to_string(),
                    ),
                    input_schema: json!({
                        "type": "object",
                        "properties": {}
                    }),
                    handler: Box::new(move |args| Self::clear_index_handler(idx.clone(), args)),
                }
            },
        ]
    }
}

pub struct ToolDef {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    pub handler: Box<
        dyn Fn(
                serde_json::Value,
            ) -> anyhow::Result<
                modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult,
            > + Send
            + Sync
            + 'static,
    >,
}
