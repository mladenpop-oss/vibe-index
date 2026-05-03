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

    fn index_file_handler(
        index: Arc<Mutex<VibeIndex>>,
        args: serde_json::Value,
    ) -> anyhow::Result<modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult>
    {
        let file_path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");

        if file_path.is_empty() || content.is_empty() {
            return Ok(
                modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult {
                    content: vec![
                        modelcontextprotocol_server::mcp_protocol::types::tool::ToolContent::Text {
                            text: "file_path and content are required".to_string(),
                        },
                    ],
                    is_error: Some(true),
                },
            );
        }

        let index_clone = Arc::clone(&index);
        let result = tokio::task::block_in_place(|| {
            let mut idx = index_clone.blocking_lock();
            let token_count = content
                .split(|c: char| !c.is_alphanumeric())
                .filter(|s| !s.is_empty())
                .count();
            idx.add_file(file_path, content);
            (
                token_count,
                idx.unique_tokens(),
                idx.total_positions(),
                idx.file_index.files.len(),
            )
        });

        Ok(
            modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult {
                content: vec![
                    modelcontextprotocol_server::mcp_protocol::types::tool::ToolContent::Text {
                        text: format!(
                            "Indexed file '{}': {} tokens ({} unique tokens total, {} positions, {} files indexed)",
                            file_path, result.0, result.1, result.2, result.3
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
            let mut sorted_results: Vec<&vibe_index::MatchResult> = results.iter().collect();
            sorted_results.sort_by(|a, b| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let mut grouped: Vec<(String, Vec<&vibe_index::MatchResult>)> = Vec::new();
            for r in sorted_results {
                let key = r
                    .file_path
                    .clone()
                    .unwrap_or_else(|| "(unknown)".to_string());
                match grouped.iter_mut().find(|(k, _)| k == &key) {
                    Some((_, matches)) => matches.push(r),
                    None => grouped.push((key, vec![r])),
                }
            }

            let mut lines = Vec::new();
            for (file, matches) in &grouped {
                lines.push(format!("=== {} ({} matches) ===", file, matches.len()));
                for r in matches {
                    let line_info = match (&r.line_number, &r.line_content) {
                        (Some(ln), Some(lc)) => format!("line {}: {}", ln, lc.trim()),
                        _ => format!("POS {}", r.position),
                    };
                    lines.push(format!(
                        "  [POS {}] conf={:.2} | {}",
                        r.position, r.confidence, line_info
                    ));
                }
                lines.push(String::new());
            }
            lines.join("\n")
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
            let mut sorted_results: Vec<&vibe_index::MatchResult> = results.iter().collect();
            sorted_results.sort_by(|a, b| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let mut grouped: Vec<(String, Vec<&vibe_index::MatchResult>)> = Vec::new();
            for r in sorted_results {
                let key = r
                    .file_path
                    .clone()
                    .unwrap_or_else(|| "(unknown)".to_string());
                match grouped.iter_mut().find(|(k, _)| k == &key) {
                    Some((_, matches)) => matches.push(r),
                    None => grouped.push((key, vec![r])),
                }
            }

            let mut lines = Vec::new();
            for (file, matches) in &grouped {
                lines.push(format!("=== {} ({} matches) ===", file, matches.len()));
                for r in matches {
                    let line_info = match (&r.line_number, &r.line_content) {
                        (Some(ln), Some(lc)) => format!("line {}: {}", ln, lc.trim()),
                        _ => format!("POS {}", r.position),
                    };
                    lines.push(format!(
                        "  [POS {}] conf={:.2} | {}",
                        r.position, r.confidence, line_info
                    ));
                }
                lines.push(String::new());
            }
            lines.join("\n")
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
            let mut sorted_results: Vec<&vibe_index::MatchResult> = results.iter().collect();
            sorted_results.sort_by(|a, b| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let mut grouped: Vec<(String, Vec<&vibe_index::MatchResult>)> = Vec::new();
            for r in sorted_results {
                let key = r
                    .file_path
                    .clone()
                    .unwrap_or_else(|| "(unknown)".to_string());
                match grouped.iter_mut().find(|(k, _)| k == &key) {
                    Some((_, matches)) => matches.push(r),
                    None => grouped.push((key, vec![r])),
                }
            }

            let mut lines = Vec::new();
            lines.push(format!(
                "Found {} matches across {} files:\n",
                results.len(),
                grouped.len()
            ));
            for (file, matches) in &grouped {
                lines.push(format!("=== {} ({} matches) ===", file, matches.len()));
                for r in matches.iter().take(5) {
                    let line_info = match (&r.line_number, &r.line_content) {
                        (Some(ln), Some(lc)) => format!("line {}: {}", ln, lc.trim()),
                        _ => format!("POS {}", r.position),
                    };
                    lines.push(format!(
                        "  [POS {}] conf={:.2} | {}",
                        r.position, r.confidence, line_info
                    ));
                }
                if matches.len() > 5 {
                    lines.push(format!("  ... and {} more matches", matches.len() - 5));
                }
                lines.push(String::new());
            }
            lines.join("\n")
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
        let (positions, unique, mem_bytes, file_stats) = tokio::task::block_in_place(|| {
            let idx = index_clone.blocking_lock();
            let fs = idx.file_index.stats();
            (
                idx.total_positions(),
                idx.unique_tokens(),
                idx.estimated_memory_bytes(),
                fs,
            )
        });

        let mem_kb = mem_bytes as f64 / 1024.0;
        let file_list: String = file_stats
            .files
            .iter()
            .map(|(path, count)| format!("  - {}: {} tokens", path, count))
            .collect::<Vec<_>>()
            .join("\n");

        let output = format!(
            "Total positions: {}\nUnique tokens: {}\nMemory: {:.2} KB ({} bytes)\nFiles indexed: {}\n\n{}\nTotal indexed tokens: {}",
            positions, unique, mem_kb, mem_bytes,
            file_stats.total_files,
            file_list,
            file_stats.total_tokens
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

    fn get_file_content_handler(
        index: Arc<Mutex<VibeIndex>>,
        args: serde_json::Value,
    ) -> anyhow::Result<modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult>
    {
        let file_path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");

        if file_path.is_empty() {
            return Ok(
                modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult {
                    content: vec![
                        modelcontextprotocol_server::mcp_protocol::types::tool::ToolContent::Text {
                            text: "file_path is required".to_string(),
                        },
                    ],
                    is_error: Some(true),
                },
            );
        }

        let index_clone = Arc::clone(&index);
        let result = tokio::task::block_in_place(|| {
            let idx = index_clone.blocking_lock();
            idx.file_index
                .files
                .iter()
                .find(|f| f.path == file_path)
                .map(|f| f.content.clone())
        });

        match result {
            Some(content) => Ok(
                modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult {
                    content: vec![
                        modelcontextprotocol_server::mcp_protocol::types::tool::ToolContent::Text {
                            text: content,
                        },
                    ],
                    is_error: None,
                },
            ),
            None => Ok(
                modelcontextprotocol_server::mcp_protocol::types::tool::ToolCallResult {
                    content: vec![
                        modelcontextprotocol_server::mcp_protocol::types::tool::ToolContent::Text {
                            text: format!("File '{}' not found in index", file_path),
                        },
                    ],
                    is_error: Some(true),
                },
            ),
        }
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
                    name: "index_file".to_string(),
                    description: Some("Index a file with path metadata. This is the preferred method for indexing source code files as it preserves file boundaries, enabling file-level grouping and line number lookup in search results.".to_string()),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "file_path": {
                                "type": "string",
                                "description": "The file path (e.g., 'src/lib.rs')"
                            },
                            "content": {
                                "type": "string",
                                "description": "The full file content"
                            }
                        },
                        "required": ["file_path", "content"]
                    }),
                    handler: Box::new(move |args| Self::index_file_handler(idx.clone(), args)),
                }
            },
            {
                let idx = Arc::clone(&self.index);
                ToolDef {
                    name: "phrase_search".to_string(),
                    description: Some("Search for an exact phrase in the indexed text. Results are grouped by file with line numbers and line content. Returns matches with position, confidence, file path, line number, and line content.".to_string()),
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
                    description: Some("Search with typo tolerance. Finds tokens that are similar to the query using Levenshtein distance. Results grouped by file with line info. Uses bigram prefiltering for speed (97% fewer computations).".to_string()),
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
                    description: Some("Unified natural language search. Parses NL query into search phrases, runs phrase search + fuzzy search, merges and ranks results by confidence. Results grouped by file with line numbers. Best for queries like 'where is the authenticate function'.".to_string()),
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
                    description: Some("Get index statistics: total positions, unique tokens, estimated memory usage, file count, and per-file token counts.".to_string()),
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
            {
                let idx = Arc::clone(&self.index);
                ToolDef {
                    name: "get_file_content".to_string(),
                    description: Some("Get the content of an indexed file by path. Returns the full file content that was stored during indexing.".to_string()),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "file_path": {
                                "type": "string",
                                "description": "The file path as indexed (e.g., 'src/auth.rs')"
                            }
                        },
                        "required": ["file_path"]
                    }),
                    handler: Box::new(move |args| Self::get_file_content_handler(idx.clone(), args)),
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
