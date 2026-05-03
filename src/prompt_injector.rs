use crate::MatchResult;
use crate::VibeIndex;
use std::time::Instant;

/// Vibe Index Prompt Injector - bridges the Vibe Index with LLM backends
pub struct PromptInjector {
    index: VibeIndex,
    /// Maximum number of context tokens to inject
    max_context_tokens: usize,
    /// Minimum confidence threshold for inclusion
    min_confidence: f64,
    /// Number of tokens to include on each side of a match
    context_window_size: usize,
}

#[derive(Debug)]
pub struct InjectedContext {
    pub prompt_tokens: Vec<String>,
    pub original_token_count: usize,
    pub filtered_token_count: usize,
    pub reduction_percent: f64,
    pub matches: Vec<MatchResult>,
    pub latency_ms: f64,
}

impl PromptInjector {
    pub fn new(max_context_tokens: usize, min_confidence: f64) -> Self {
        Self {
            index: VibeIndex::new(),
            max_context_tokens,
            min_confidence,
            context_window_size: 5,
        }
    }

    /// Create a new PromptInjector with a custom context window size.
    pub fn with_context_window(
        max_context_tokens: usize,
        min_confidence: f64,
        window_size: usize,
    ) -> Self {
        Self {
            index: VibeIndex::new(),
            max_context_tokens,
            min_confidence,
            context_window_size: window_size,
        }
    }

    pub fn add_token(&mut self, token: &str) {
        self.index.add_token(token);
    }

    /// Build an optimized prompt by injecting only relevant context
    /// This is the core function that replaces naive RAG context injection
    pub fn build_prompt(
        &mut self,
        user_query: &str,
        full_context_tokens: &[String],
        search_queries: &[Vec<String>],
    ) -> InjectedContext {
        let start = Instant::now();
        let original_count = full_context_tokens.len();

        // Step 1: Search for relevant positions
        let mut all_matches: Vec<MatchResult> = Vec::new();
        for query in search_queries {
            let results = self.index.phrase_search(query);
            all_matches.extend(results);
        }

        // Step 2: Also do fuzzy search for typos in user query
        let fuzzy_results = self.index.fuzzy_search(user_query, 2);
        all_matches.extend(fuzzy_results);

        // Step 3: Filter by confidence
        all_matches.retain(|m| m.confidence >= self.min_confidence);

        // Step 4: Sort by position and extract relevant context windows
        all_matches.sort_by_key(|m| m.position);
        let relevant_positions =
            self._extract_relevant_positions(&all_matches, full_context_tokens.len());

        // Step 5: Build optimized prompt
        let prompt_tokens =
            self._build_optimized_prompt(&relevant_positions, full_context_tokens, user_query);

        let filtered_count = prompt_tokens.len();
        let reduction = if original_count > 0 {
            (1.0 - (filtered_count as f64 / original_count as f64)) * 100.0
        } else {
            0.0
        };

        let latency = start.elapsed().as_secs_f64() * 1000.0;

        println!(
            "[PROMPT] Original: {} tokens -> Filtered: {} tokens ({}% reduction)",
            original_count, filtered_count, reduction
        );
        println!("[PROMPT] Found {} relevant matches", all_matches.len());
        println!("[PROMPT] Build latency: {:.2}ms", latency);

        InjectedContext {
            prompt_tokens,
            original_token_count: original_count,
            filtered_token_count: filtered_count,
            reduction_percent: reduction,
            matches: all_matches,
            latency_ms: latency,
        }
    }

    fn _extract_relevant_positions(
        &self,
        matches: &[MatchResult],
        context_len: usize,
    ) -> Vec<usize> {
        let mut positions = std::collections::BTreeSet::new();

        for m in matches {
            let window_size = self.context_window_size;
            for offset in -(window_size as i64)..=(window_size as i64) {
                let pos = (m.position as i64 + offset) as usize;
                if pos < context_len {
                    positions.insert(pos);
                }
            }
        }

        // Limit to max_context_tokens
        let mut sorted: Vec<usize> = positions.into_iter().collect();
        if sorted.len() > self.max_context_tokens {
            // Keep every Nth position to stay within budget
            let step = sorted.len() / self.max_context_tokens;
            if step > 1 {
                sorted = sorted.into_iter().step_by(step).collect();
            }
        }

        sorted
    }

    fn _build_optimized_prompt(
        &self,
        positions: &[usize],
        full_context: &[String],
        user_query: &str,
    ) -> Vec<String> {
        if positions.is_empty() {
            return vec![
                "<context>".to_string(),
                "(no relevant context found)".to_string(),
                "</context>".to_string(),
                "<query>".to_string(),
                user_query.to_string(),
                "</query>".to_string(),
            ];
        }

        let mut tokens = Vec::new();

        // Add context header
        tokens.push("<context>".to_string());

        // Add selected context tokens with position markers
        for pos in positions {
            if *pos < full_context.len() {
                tokens.push(format!("[{}]", pos));
                tokens.push(full_context[*pos].clone());
            }
        }

        tokens.push("</context>".to_string());

        // Add query
        tokens.push("<query>".to_string());
        tokens.push(user_query.to_string());
        tokens.push("</query>".to_string());

        tokens
    }

    /// Calculate estimated KV cache savings
    pub fn estimate_kv_cache_savings(&self, context: &InjectedContext) -> f64 {
        // Each token = 1 KV entry per layer
        // Each entry: 2 (q + k) * hidden_size * 2 bytes (fp16)
        // Assuming 4096 hidden, 2 bytes per element
        let layers = 32;
        let hidden_size = 4096;
        let bytes_per_entry = 2.0 * hidden_size as f64 * 2.0; // q + k, fp16
        let saved_entries = (context.original_token_count - context.filtered_token_count) as f64;
        (saved_entries * layers as f64 * bytes_per_entry) / (1024.0 * 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_injector_basic() {
        let mut injector = PromptInjector::new(100, 0.0);
        injector.add_token("fn");
        injector.add_token("main");
        injector.add_token("(");
        injector.add_token(")");
        injector.add_token("{");
        injector.add_token("}");

        let context = vec![
            "fn".into(),
            "main".into(),
            "(".into(),
            ")".into(),
            "{".into(),
            "let".into(),
            "x".into(),
            "= 42;".into(),
            "}".into(),
        ];

        let search_queries = vec![vec!["fn".into(), "main".into()]];
        let result = injector.build_prompt("find main function", &context, &search_queries);

        assert!(!result.prompt_tokens.is_empty());
        assert!(result.prompt_tokens.iter().any(|t| t == "<context>"));
        assert!(result.prompt_tokens.iter().any(|t| t == "<query>"));
        assert!(result.matches.len() >= 1);
    }

    #[test]
    fn test_prompt_injector_no_matches() {
        let mut injector = PromptInjector::new(100, 0.0);
        injector.add_token("fn");
        injector.add_token("main");

        let context = vec!["fn".into(), "main".into(), "()".into()];
        let search_queries = vec![vec!["nonexistent".into(), "token".into()]];
        let result = injector.build_prompt("search for missing", &context, &search_queries);

        assert!(result
            .prompt_tokens
            .iter()
            .any(|t| t == "(no relevant context found)"));
        assert_eq!(result.matches.len(), 0);
    }

    #[test]
    fn test_prompt_injector_confidence_filter() {
        let mut injector = PromptInjector::new(100, 0.9);
        injector.add_token("fn");
        injector.add_token("authenticate");

        let context = vec![
            "fn".into(),
            "authenticate".into(),
            "(".into(),
            ")".into(),
            "{".into(),
            "}".into(),
        ];

        let search_queries = vec![vec!["fn".into(), "authenticate".into()]];
        let result = injector.build_prompt("authenticate function", &context, &search_queries);

        assert!(!result.matches.is_empty());
        for m in &result.matches {
            assert!(m.confidence >= 0.9);
        }
    }

    #[test]
    fn test_prompt_injector_context_window() {
        let mut injector = PromptInjector::with_context_window(100, 0.0, 2);
        injector.add_token("fn");
        injector.add_token("main");

        let context: Vec<String> = (0..20).map(|i| format!("token{}", i)).collect();
        let context_with_fn = {
            let mut c = context.clone();
            c[5] = "fn".into();
            c[6] = "main".into();
            c
        };

        let search_queries = vec![vec!["fn".into(), "main".into()]];
        let result = injector.build_prompt("find fn main", &context_with_fn, &search_queries);

        assert!(!result.prompt_tokens.is_empty());
    }

    #[test]
    fn test_kv_cache_savings() {
        let injector = PromptInjector::new(100, 0.0);
        let context = InjectedContext {
            prompt_tokens: vec!["fn".into(), "main".into()],
            original_token_count: 1000,
            filtered_token_count: 100,
            reduction_percent: 90.0,
            matches: vec![],
            latency_ms: 0.5,
        };

        let savings = injector.estimate_kv_cache_savings(&context);
        assert!(savings > 0.0);
    }

    #[test]
    fn test_prompt_injector_empty_context() {
        let mut injector = PromptInjector::new(100, 0.0);
        let context: Vec<String> = vec![];
        let search_queries: Vec<Vec<String>> = vec![];
        let result = injector.build_prompt("empty search", &context, &search_queries);

        assert_eq!(result.original_token_count, 0);
        assert_eq!(result.reduction_percent, 0.0);
    }
}
