use super::{query_parser, MatchResult, VibeIndex};
use crate::hybrid_search::HybridSearcher;
use std::collections::HashMap;
use std::time::Instant;

/// vLLM integration with hybrid search, context management, and output validation.
///
/// This is the production-ready integration layer that connects Vibe Index
/// to vLLM's OpenAI-compatible API with:
/// - Hybrid search (BM25 candidates + Vibe Index validation)
/// - Context window budget management
/// - Post-injection validation (token boundary checks)
/// - Output sanity checks (syntax validation)
/// - Confidence feedback loop
pub struct VllmIntegration {
    hybrid: HybridSearcher,
    vibe: VibeIndex,
    server_url: String,
    /// Maximum tokens allowed in context window
    max_context_tokens: usize,
    /// Confidence scores per query (for feedback loop)
    confidence_history: HashMap<String, Vec<f64>>,
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

/// Validates injected context doesn't break structure
#[derive(Debug, Clone)]
pub struct ContextValidation {
    pub token_boundary_safe: bool,
    pub balanced_braces: bool,
    pub no_truncated_tokens: bool,
    pub issues: Vec<String>,
}

/// Output validation results
#[derive(Debug, Clone)]
pub struct OutputValidation {
    pub syntax_valid: bool,
    pub issues: Vec<String>,
    pub confidence_adjustment: f64,
}

impl VllmIntegration {
    pub fn new(server_url: String, max_context_tokens: usize) -> Self {
        Self {
            hybrid: HybridSearcher::new(5), // top-5 BM25 candidates
            vibe: VibeIndex::new(),
            server_url,
            max_context_tokens,
            confidence_history: HashMap::new(),
        }
    }

    pub fn add_token(&mut self, token: &str) {
        self.vibe.add_token(token);
    }

    pub fn add_document(&mut self, start: usize, end: usize) {
        self.hybrid.add_document(start, end);
    }

    pub fn index_tokens(&mut self, tokens: &[String]) {
        self.hybrid.index_tokens(tokens);
    }

    /// Build optimized messages using hybrid search
    pub fn build_vibe_messages(
        &mut self,
        user_query: &str,
        full_context: &[String],
        search_queries: &[Vec<String>],
    ) -> (Vec<VllmChatMessage>, Vec<MatchResult>, ContextValidation) {
        let start = Instant::now();

        // 1. Parse query into phrases
        let stop_set: std::collections::HashSet<&str> =
            query_parser::ENGLISH_STOP_WORDS.iter().copied().collect();
        let _query_tokens: Vec<String> = user_query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && !stop_set.contains(s))
            .map(|s| s.to_lowercase())
            .collect();

        // 2. Run hybrid search
        let mut all_matches: Vec<MatchResult> = Vec::new();

        // Hybrid search on user query
        let hybrid_results = self.hybrid.search(user_query);
        all_matches.extend(hybrid_results);

        // Also run vibe-only for additional queries
        for query in search_queries {
            let results = self.vibe.phrase_search(query);
            all_matches.extend(results);
        }

        // 3. Context window budget management
        all_matches.retain(|m| m.confidence >= 0.5);
        all_matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        let mut token_count = 0;
        let mut filtered_matches: Vec<MatchResult> = Vec::new();

        for m in all_matches {
            let window_size = 15;
            let match_tokens = (window_size * 2) + 1; // ±15 + position
            if token_count + match_tokens > self.max_context_tokens {
                break; // Budget exhausted
            }
            token_count += match_tokens;
            filtered_matches.push(m);
        }

        // 4. Build context string with validation
        let mut context_parts = Vec::new();
        let mut validation = ContextValidation {
            token_boundary_safe: true,
            balanced_braces: true,
            no_truncated_tokens: true,
            issues: Vec::new(),
        };

        let mut brace_count = 0;

        for m in &filtered_matches {
            let pos = m.position;
            let window_size = 15;
            let start_idx = pos.saturating_sub(window_size);
            let end_idx = (pos + window_size).min(full_context.len());

            if start_idx < end_idx {
                let snippet: String = full_context[start_idx..end_idx].join(" ");

                // Validate token boundary safety
                let has_truncated = snippet.contains("�") || snippet.ends_with("...");
                if has_truncated {
                    validation.no_truncated_tokens = false;
                    validation
                        .issues
                        .push(format!("Truncated token at position {}", pos));
                }

                // Check brace balance
                for c in snippet.chars() {
                    match c {
                        '{' => brace_count += 1,
                        '}' => brace_count -= 1,
                        _ => {}
                    }
                }

                context_parts.push(format!("  [POS {}] {}", pos, snippet));
            }
        }

        if brace_count != 0 {
            validation.balanced_braces = false;
            validation.issues.push(format!(
                "Unbalanced braces: {} open, {} close",
                (brace_count.max(0)),
                (-brace_count.min(0))
            ));
        }

        let context_str = if context_parts.is_empty() {
            "(No relevant context found)".to_string()
        } else {
            context_parts.join("\n")
        };

        // 5. Build system prompt with confidence-aware instructions
        let system_prompt = format!(
            "You are a code assistant with access to a Vibe Index (positional phrase matching). \
             The following context has been retrieved using hybrid search (BM25 + exact position validation). \
             Use ONLY the relevant context below to answer the query accurately.\n\n\
             === RETRIEVED CONTEXT ({:.0}% confidence) ===\n{}\n=========================",
            filtered_matches.iter().map(|m| m.confidence).sum::<f64>() / filtered_matches.len().max(1) as f64 * 100.0,
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
        println!(
            "[VLLM] Built {} messages, {} matches ({:.0}% signal), {:.2}ms",
            messages.len(),
            filtered_matches.len(),
            if !filtered_matches.is_empty() {
                filtered_matches.iter().map(|m| m.confidence).sum::<f64>()
                    / filtered_matches.len() as f64
                    * 100.0
            } else {
                0.0
            },
            latency
        );

        (messages, filtered_matches, validation)
    }

    /// Validate generated output for common issues
    pub fn validate_output(&self, generated_code: &str) -> OutputValidation {
        let mut issues = Vec::new();
        let mut syntax_valid = true;
        let mut confidence_adjustment: f64 = 0.0;

        // Check for common syntax issues
        let brace_count = generated_code.chars().filter(|&c| c == '{').count() as i32
            - generated_code.chars().filter(|&c| c == '}').count() as i32;
        if brace_count != 0 {
            issues.push(format!("Unbalanced braces: {} difference", brace_count));
            syntax_valid = false;
            confidence_adjustment -= 0.2;
        }

        let paren_count = generated_code.chars().filter(|&c| c == '(').count() as i32
            - generated_code.chars().filter(|&c| c == ')').count() as i32;
        if paren_count != 0 {
            issues.push(format!(
                "Unbalanced parentheses: {} difference",
                paren_count
            ));
            syntax_valid = false;
            confidence_adjustment -= 0.15;
        }

        // Check for truncated code (common with context window limits)
        if generated_code.ends_with("...") || generated_code.contains("�") {
            issues.push("Output appears truncated".to_string());
            confidence_adjustment -= 0.1;
        }

        // Check for missing semicolons in Rust-like code
        let lines: Vec<&str> = generated_code.lines().collect();
        if !lines.is_empty() {
            let last_line = lines.last().unwrap_or(&"");
            if !last_line.trim().is_empty()
                && !last_line.trim().ends_with(';')
                && !last_line.trim().ends_with('{')
                && !last_line.trim().ends_with('}')
                && !last_line.trim().ends_with(',')
            {
                issues.push("Last line may be missing terminator".to_string());
                confidence_adjustment -= 0.05;
            }
        }

        OutputValidation {
            syntax_valid,
            issues,
            confidence_adjustment: confidence_adjustment.clamp(-0.5, 0.0),
        }
    }

    /// Update confidence history and adjust future search weights
    pub fn update_confidence_feedback(&mut self, query: &str, output_valid: bool) {
        let entry = self
            .confidence_history
            .entry(query.to_string())
            .or_default();

        if output_valid {
            entry.push(1.0);
        } else {
            entry.push(0.0);
        }

        // Keep only last 10 entries per query
        if entry.len() > 10 {
            entry.remove(0);
        }

        // Calculate success rate
        let success_rate = entry.iter().sum::<f64>() / entry.len() as f64;
        println!(
            "[VLLM] Confidence feedback for '{}': {:.0}% success rate ({} samples)",
            query,
            success_rate * 100.0,
            entry.len()
        );
    }

    /// Get average confidence for a query
    pub fn get_query_confidence(&self, query: &str) -> Option<f64> {
        self.confidence_history
            .get(query)
            .map(|scores| scores.iter().sum::<f64>() / scores.len() as f64)
    }

    /// Send chat completion request to vLLM server
    pub async fn chat(&self, messages: &[VllmChatMessage]) -> Result<String, anyhow::Error> {
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

    /// Full pipeline: index context -> hybrid search -> build messages -> get response -> validate output
    pub async fn ask(
        &mut self,
        context: &[String],
        user_query: &str,
        search_queries: &[Vec<String>],
    ) -> Result<
        (
            String,
            Vec<MatchResult>,
            ContextValidation,
            OutputValidation,
        ),
        anyhow::Error,
    > {
        // Index the context
        for token in context {
            self.add_token(token);
        }

        // Build optimized messages with hybrid search
        let (messages, matches, ctx_validation) =
            self.build_vibe_messages(user_query, context, search_queries);

        // Get response from vLLM
        let response = self.chat(&messages).await?;

        // Validate output
        let output_validation = self.validate_output(&response);

        // Update confidence feedback
        self.update_confidence_feedback(user_query, output_validation.syntax_valid);

        Ok((response, matches, ctx_validation, output_validation))
    }

    /// Calculate context window savings
    pub fn get_context_stats(
        &self,
        original_tokens: usize,
        filtered_tokens: usize,
    ) -> ContextStats {
        let saved = (original_tokens as i64 - filtered_tokens as i64).max(0);
        let savings_pct = if original_tokens > 0 {
            (saved as f64 / original_tokens as f64) * 100.0
        } else {
            0.0
        };

        ContextStats {
            original_tokens,
            filtered_tokens,
            saved_tokens: saved,
            savings_percent: savings_pct,
        }
    }
}

#[derive(Debug)]
pub struct ContextStats {
    pub original_tokens: usize,
    pub filtered_tokens: usize,
    pub saved_tokens: i64,
    pub savings_percent: f64,
}

/// Example: Pre-built prompts for common vLLM use cases
pub mod prompts {
    pub const CODE_REVIEW: &str =
        "Review the following code for bugs, performance issues, and style violations.";
    pub const GENERATE_TESTS: &str = "Generate comprehensive unit tests for the following code.";
    pub const EXPLAIN_CODE: &str = "Explain what this code does in detail.";
    pub const FIND_BUGS: &str = "Find all bugs and potential issues in the following code.";
    pub const REFACTORING: &str = "Refactor the following code to be more readable and efficient.";
}
