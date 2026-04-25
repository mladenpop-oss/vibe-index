use crate::VibeIndex;
use crate::MatchResult;
use std::time::Instant;

/// vLLM integration module
/// Connects Vibe Index to vLLM's OpenAI-compatible API
pub struct VllmIntegration {
    index: VibeIndex,
    /// vLLM server endpoint (OpenAI-compatible)
    server_url: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct VllmChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct VllmChatRequest {
    pub model: String,
    pub messages: Vec<VllmChatMessage>,
    pub temperature: f32,
    pub max_tokens: i32,
    pub top_p: f32,
    pub stream: bool,
}

#[derive(serde::Deserialize, Debug)]
pub struct VllmChatResponse {
    pub choices: Vec<VllmChoice>,
}

#[derive(serde::Deserialize, Debug)]
pub struct VllmChoice {
    pub message: VllmChatMessage,
    pub finish_reason: String,
}

impl VllmIntegration {
    pub fn new(server_url: String) -> Self {
        Self {
            index: VibeIndex::new(),
            server_url,
        }
    }

    pub fn add_token(&mut self, token: &str) {
        self.index.add_token(token);
    }

    /// Build an optimized system prompt with Vibe Index context
    pub fn build_vibe_messages(
        &mut self,
        user_query: &str,
        full_context: &[String],
        search_queries: &[Vec<String>],
    ) -> (Vec<VllmChatMessage>, Vec<MatchResult>) {
        let start = Instant::now();

        // Search for relevant positions
        let mut all_matches: Vec<MatchResult> = Vec::new();
        for query in search_queries {
            let results = self.index.phrase_search(query);
            all_matches.extend(results);
        }
        all_matches.retain(|m| m.confidence >= 0.5);
        all_matches.sort_by_key(|m| m.position);

        // Build context string
        let mut context_parts = Vec::new();
        for m in &all_matches {
            let pos = m.position;
            let window_size = 15;
            let start = pos.saturating_sub(window_size);
            let end = (pos + window_size).min(full_context.len());

            if start < end {
                let snippet: String = full_context[start..end].join(" ");
                context_parts.push(format!("  [POS {}] {}", pos, snippet));
            }
        }

        let context_str = if context_parts.is_empty() {
            "(No relevant context found)".to_string()
        } else {
            context_parts.join("\n")
        };

        let system_prompt = format!(
            "You are a code assistant with access to a Vibe Index. \
             The following context has been retrieved using positional phrase matching. \
             Use ONLY the relevant context below to answer the query accurately.\n\n\
             === RELEVANT CONTEXT ===\n{}\n=========================",
            context_str
        );

        let user_message = format!(
            "Based on the context above, please answer:\n\n{}",
            user_query
        );

        let messages = vec![
            VllmChatMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            VllmChatMessage {
                role: "user".to_string(),
                content: user_message,
            },
        ];

        let latency = start.elapsed().as_secs_f64() * 1000.0;
        println!("[VLLM] Built {} messages, {} matches, {:.2}ms",
            messages.len(), all_matches.len(), latency);

        (messages, all_matches)
    }

    /// Send chat completion request to vLLM server
    pub async fn chat(
        &self,
        messages: &[VllmChatMessage],
    ) -> Result<String, anyhow::Error> {
        let request = VllmChatRequest {
            model: "local-model".to_string(),
            messages: messages.to_vec(),
            temperature: 0.7,
            max_tokens: 1024,
            top_p: 0.95,
            stream: false,
        };

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/v1/chat/completions", self.server_url))
            .json(&request)
            .send()
            .await?
            .json::<VllmChatResponse>()
            .await?;

        if let Some(choice) = response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow::anyhow!("No response from vLLM"))
        }
    }

    /// Full pipeline: index context -> build messages -> get response
    pub async fn ask(
        &mut self,
        context: &[String],
        user_query: &str,
        search_queries: &[Vec<String>],
    ) -> Result<(String, Vec<MatchResult>), anyhow::Error> {
        // Index the context
        for token in context {
            self.add_token(token);
        }

        // Build optimized messages
        let (messages, matches) = self.build_vibe_messages(user_query, context, search_queries);

        // Get response from vLLM
        let response = self.chat(&messages).await?;

        Ok((response, matches))
    }

    /// Calculate context window savings
   pub fn get_context_stats(&self, original_tokens: usize, filtered_tokens: usize) -> ContextStats {
        let saved = (original_tokens - filtered_tokens) as i64;
        let savings_pct = if original_tokens > 0 {
            (saved as f64 / original_tokens as f64) * 100.0
        } else {
            0.0
        };

        // Estimate KV cache savings
        // Each token = 1 KV entry per layer
        // Each entry: 2 (q + k) * hidden_size * 2 bytes (fp16)
        let layers = 32_i64;
        let hidden = 4096_i64;
        let bytes_per_entry = 2 * hidden * 2; // q + k, fp16
        let saved_mb = (saved * layers * bytes_per_entry) / (1024 * 1024);

        ContextStats {
            original_tokens,
            filtered_tokens,
            saved_tokens: saved,
            savings_percent: savings_pct,
            estimated_kv_cache_saved_mb: saved_mb as f64,
        }
    }
}

#[derive(Debug)]
pub struct ContextStats {
    pub original_tokens: usize,
    pub filtered_tokens: usize,
    pub saved_tokens: i64,
    pub savings_percent: f64,
    pub estimated_kv_cache_saved_mb: f64,
}

/// Example: Pre-built prompts for common vLLM use cases
pub mod prompts {
    pub const CODE_REVIEW: &str = "Review the following code for bugs, performance issues, and style violations.";
    pub const GENERATE_TESTS: &str = "Generate comprehensive unit tests for the following code.";
    pub const EXPLAIN_CODE: &str = "Explain what this code does in detail.";
    pub const FIND_BUGS: &str = "Find all bugs and potential issues in the following code.";
    pub const REFACTORING: &str = "Refactor the following code to be more readable and efficient.";
}
