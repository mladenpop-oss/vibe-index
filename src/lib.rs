use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    pub position: usize,
    pub context: String,
    pub confidence: f64,
}

pub struct VibeIndex {
    pub token_positions: HashMap<String, RoaringBitmap>,
    pub token_sequence: Vec<String>,
    position: usize,
}

impl Default for VibeIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl VibeIndex {
    pub fn new() -> Self {
        Self {
            token_positions: HashMap::new(),
            token_sequence: Vec::new(),
            position: 0,
        }
    }

    pub fn add_token(&mut self, token: &str) {
        self.token_positions
            .entry(token.to_string())
            .or_default()
            .push(self.position as u32);
        self.token_sequence.push(token.to_string());
        self.position += 1;
    }

    pub fn phrase_search(&self, query: &[String]) -> Vec<MatchResult> {
        if query.is_empty() {
            return Vec::new();
        }

        // Collect bitmaps with their query indices
        let mut masks: Vec<(usize, &RoaringBitmap)> = Vec::new();
        for (i, token) in query.iter().enumerate() {
            match self.token_positions.get(token) {
                Some(bitmap) => masks.push((i, bitmap)),
                None => return Vec::new(),
            }
        }

        // Find smallest bitmap to use as anchor (fewest iterations)
        let anchor_idx = masks
            .iter()
            .min_by_key(|(_, b)| b.len())
            .map(|(i, _)| *i)
            .unwrap_or(0);
        let anchor_bitmap = masks.iter().find(|(i, _)| *i == anchor_idx).unwrap().1;

        // For each position in the smallest bitmap, check if all other positions align
        let mut result = RoaringBitmap::new();
        for pos in anchor_bitmap.iter() {
            let mut matches = true;
            for (query_offset, (_, bitmap)) in masks.iter().enumerate() {
                // Calculate expected position: anchor_pos + (query_offset - anchor_query_index)
                let expected_pos = pos as i64 + (query_offset as i64 - anchor_idx as i64);
                if expected_pos < 0 || !bitmap.contains(expected_pos as u32) {
                    matches = false;
                    break;
                }
            }
            if matches {
                // Use the first query token's position as the match position
                let _first_bitmap = masks.first().unwrap().1;
                let first_pos = pos as i64 - (anchor_idx as i64);
                if first_pos >= 0 {
                    result.push(first_pos as u32);
                }
            }
        }

        result
            .iter()
            .map(|pos| {
                let pos = pos as usize;
                let query_len = query.len();
                let context_start = pos.saturating_sub(3);
                let context_end = (pos + query_len).min(self.token_sequence.len());
                let context_tokens: Vec<&str> = self.token_sequence[context_start..context_end]
                    .iter()
                    .map(|s| s.as_str())
                    .collect();
                let context = if context_end - context_start > 1 {
                    format!(
                        "[POS {}] '{}' (context: ... {})",
                        pos,
                        query.join(" "),
                        context_tokens.join(" ")
                    )
                } else {
                    format!("[POS {}] matched: '{}'", pos, query.join(" "))
                };
                MatchResult {
                    position: pos,
                    context,
                    confidence: 1.0,
                }
            })
            .collect()
    }

    pub fn fuzzy_search(&self, query: &str, max_distance: usize) -> Vec<MatchResult> {
        let mut results = Vec::new();
        for (stored_token, bitmap) in &self.token_positions {
            let distance = levenshtein(query, stored_token);
            if distance <= max_distance && distance > 0 {
                for pos in bitmap.iter() {
                    results.push(MatchResult {
                        position: pos as usize,
                        context: format!(
                            "[POS {}] fuzzy: '{}' (dist={}) -> '{}'",
                            pos, stored_token, distance, query
                        ),
                        confidence: 1.0 - (distance as f64 / (max_distance as f64 + 1.0)),
                    });
                }
            }
        }
        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        results
    }

    /// Unified search: natural language query → phrase search + fuzzy search → merged results
    ///
    /// This is the high-level API. It:
    /// 1. Parses the query into search phrases using query_parser
    /// 2. Runs phrase_search on each phrase
    /// 3. Runs fuzzy_search for typo tolerance
    /// 4. Merges all results, deduplicates by position, sorts by confidence
    pub fn search(&self, query: &str) -> Vec<MatchResult> {
        let mut all_results: Vec<MatchResult> = Vec::new();
        let mut seen_positions: std::collections::HashSet<usize> = std::collections::HashSet::new();

        // 1. Parse query into phrases and run phrase_search on each
        let phrases = query_parser::parse_query(query);
        for phrase in &phrases {
            let results = self.phrase_search(phrase);
            for mut r in results {
                r.confidence = 0.95; // phrase matches get high confidence
                if seen_positions.insert(r.position) {
                    all_results.push(r);
                }
            }
        }

        // 2. Run fuzzy search for each significant word in the query
        let stop_set: std::collections::HashSet<&str> =
            query_parser::ENGLISH_STOP_WORDS.iter().copied().collect();
        let words: Vec<&str> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect();

        for word in &words {
            if stop_set.contains(*word) || word.len() <= 1 {
                continue;
            }
            let fuzzy_results = self.fuzzy_search(word.to_lowercase().as_str(), 1);
            for mut r in fuzzy_results {
                r.confidence *= 0.5; // fuzzy matches get lower confidence
                if seen_positions.insert(r.position) {
                    all_results.push(r);
                }
            }
        }

        // 3. Sort by confidence (highest first), then by position
        all_results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.position.cmp(&b.position))
        });

        all_results
    }

    pub fn total_positions(&self) -> usize {
        self.position
    }
    pub fn unique_tokens(&self) -> usize {
        self.token_positions.len()
    }
    pub fn estimated_memory_bytes(&self) -> usize {
        let token_seq_bytes = self.token_sequence.iter()
            .map(|s| s.capacity() + 24)
            .sum::<usize>();
        let bitmap_bytes = self.token_positions.iter()
            .map(|(k, bitmap)| {
                k.capacity() + 24 + bitmap.serialized_size()
            })
            .sum::<usize>();
        token_seq_bytes + bitmap_bytes
    }
}

#[allow(clippy::needless_range_loop)]
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let la = a_chars.len();
    let lb = b_chars.len();
    if la == 0 {
        return lb;
    }
    if lb == 0 {
        return la;
    }
    let mut matrix = vec![vec![0usize; lb + 1]; la + 1];
    for i in 0..=la {
        matrix[i][0] = i;
    }
    for j in 0..=lb {
        matrix[0][j] = j;
    }
    for i in 1..=la {
        for j in 1..=lb {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            matrix[i][j] = min(
                matrix[i - 1][j] + 1,
                min(matrix[i][j - 1] + 1, matrix[i - 1][j - 1] + cost),
            );
        }
    }
    matrix[la][lb]
}

fn min(a: usize, b: usize) -> usize {
    if a < b {
        a
    } else {
        b
    }
}

pub mod bm25;
pub mod hot_cold;
pub mod hybrid_search;
pub mod llama_cpp;
pub mod persistent_storage;
pub mod query_parser;
pub mod vllm;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_token_index() {
        let mut index = VibeIndex::new();
        index.add_token("fn");
        index.add_token("main");
        index.add_token("(");
        assert_eq!(index.total_positions(), 3);
        assert_eq!(index.unique_tokens(), 3);
    }

    #[test]
    fn test_phrase_search_exact() {
        let mut index = VibeIndex::new();
        for token in ["fn", "main", "(", ")", "{"] {
            index.add_token(token);
        }
        let results = index.phrase_search(&["fn".into(), "main".into()]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].position, 0);
    }

    #[test]
    fn test_phrase_search_not_found() {
        let mut index = VibeIndex::new();
        for token in ["fn", "main", "(", ")", "{"] {
            index.add_token(token);
        }
        let results = index.phrase_search(&["fn".into(), "not_here".into()]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_fuzzy_search() {
        let mut index = VibeIndex::new();
        index.add_token("execute");
        let results = index.fuzzy_search("execut", 2);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_phrase_search_multiple_matches() {
        let mut index = VibeIndex::new();
        for token in ["fn", "a", "fn", "b", "fn", "a"] {
            index.add_token(token);
        }
        let results = index.phrase_search(&["fn".into(), "a".into()]);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_phrase_match() {
        let mut index = VibeIndex::new();
        for token in [
            "fn",
            "fetch",
            "data",
            "(",
            "db",
            ":",
            "&str",
            ")",
            "→",
            "Result",
            "{",
            "let",
            "conn",
            "=",
            "db.connect()",
            ";",
            "Ok",
            "(",
            "conn",
            ")",
            "}",
        ] {
            index.add_token(token);
        }
        let results = index.search("where is the fetch data function");
        assert!(!results.is_empty(), "Should find fetch data");
        assert!(
            results[0].confidence > 0.9,
            "Phrase match should have high confidence"
        );
    }

    #[test]
    fn test_search_fuzzy_match() {
        let mut index = VibeIndex::new();
        for token in ["execute", "query", "database"] {
            index.add_token(token);
        }
        let results = index.search("where is the execut method");
        assert!(!results.is_empty(), "Should find execute via fuzzy match");
        assert!(
            results.iter().any(|r| r.confidence < 0.9),
            "Fuzzy match should have lower confidence"
        );
    }

    #[test]
    fn test_search_combined() {
        let mut index = VibeIndex::new();
        for token in [
            "fn",
            "main",
            "(",
            ")",
            "{",
            "let",
            "mut",
            "cache",
            "=",
            "HashMap::new",
            "(",
            ")",
            ";",
            "cache",
            ".",
            "insert",
            "(",
            "\"key\"",
            ",",
            "42",
            ")",
            ";",
            "println!",
            "(",
            "\"done\"",
            ")",
            ";",
            "}",
            "fn",
            "process_data",
            "(",
            "data",
            ":",
            "&Vec",
            "<String>",
            ")",
            "→",
            "Result",
            "<()",
            "Error>",
            "{",
            "for",
            "item",
            "in",
            "data",
            "{",
            "results",
            ".",
            "push",
            "(",
            "item",
            ".",
            "to_uppercase",
            "(",
            ")",
            ")",
            ";",
            "}",
            "Ok",
            "(",
            "()",
            ")",
            "}",
        ] {
            index.add_token(token);
        }
        let results = index.search("how does the process_data function work");
        assert!(!results.is_empty(), "Should find process_data");
        // Should be sorted by confidence
        for i in 1..results.len() {
            assert!(
                results[i - 1].confidence >= results[i].confidence,
                "Results should be sorted by confidence"
            );
        }
    }

    #[test]
    fn test_search_empty_query() {
        let mut index = VibeIndex::new();
        for token in [
            "fn", "main", "(", ")", "{", "}", "fn", "add", "(", "a", "b", ")", "→", "i32", "{",
            "a", "+", "b", "}",
        ] {
            index.add_token(token);
        }
        let results = index.search("the a is");
        // Should return empty or very low confidence results (all stop words)
        assert!(
            results.is_empty() || results.iter().all(|r| r.confidence < 0.1),
            "Stop-word-only query should return minimal results"
        );
    }
}
