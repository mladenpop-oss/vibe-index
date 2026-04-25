use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Represents a segment of tokens stored in the cold layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdSegment {
    /// Starting position of this segment
    pub start_pos: u32,
    /// Number of tokens in this segment
    pub token_count: u32,
    /// Token sequence (JSON-serialized, stored as string for simplicity)
    pub tokens_json: String,
}

/// Hot layer: recent tokens kept in memory for instant access
#[derive(Debug)]
pub struct HotLayer {
    /// Tokens in the hot buffer
    pub tokens: Vec<String>,
    /// Token position bitmaps (only for hot layer)
    pub token_positions: HashMap<String, RoaringBitmap>,
    /// Base position offset (where this hot layer starts globally)
    pub base_offset: u32,
}

impl HotLayer {
    pub fn new(base_offset: u32) -> Self {
        Self {
            tokens: Vec::new(),
            token_positions: HashMap::new(),
            base_offset,
        }
    }

    pub fn add_token(&mut self, token: &str) {
        let global_pos = self.tokens.len() as u32 + self.base_offset;
        self.token_positions
            .entry(token.to_string())
            .or_default()
            .push(global_pos);
        self.tokens.push(token.to_string());
    }

    pub fn get_context(&self, pos: usize, query_len: usize, context_window: usize) -> String {
        let start = pos.saturating_sub(context_window);
        let end = (pos + query_len).min(self.tokens.len());
        if start < end {
            self.tokens[start..end].join(" ")
        } else {
            String::new()
        }
    }
}

/// Cold layer: compressed segments stored on disk
#[derive(Debug)]
pub struct ColdLayer {
    /// Directory for cold storage
    pub storage_dir: String,
    /// All cold segments
    pub segments: Vec<ColdSegment>,
}

impl ColdLayer {
    pub fn new(storage_dir: &str) -> Self {
        let dir = Path::new(storage_dir);
        if !dir.exists() {
            fs::create_dir_all(dir).expect("Failed to create cold storage directory");
        }
        Self {
            storage_dir: storage_dir.to_string(),
            segments: Vec::new(),
        }
    }

    /// Flush hot layer to cold storage
    pub fn flush_segment(&mut self, hot_layer: &HotLayer) {
        if hot_layer.tokens.is_empty() {
            return;
        }

        let tokens_json =
            serde_json::to_string(&hot_layer.tokens).expect("Failed to serialize tokens");

        let segment = ColdSegment {
            start_pos: hot_layer.base_offset,
            token_count: hot_layer.tokens.len() as u32,
            tokens_json,
        };

        self.segments.push(segment.clone());
        self.save_segment(&segment);
    }

    /// Save segment to disk
    fn save_segment(&self, segment: &ColdSegment) {
        let file_path = format!("{}/segment_{}.bin", self.storage_dir, segment.start_pos);
        let serialized = serde_json::to_vec(segment).expect("Failed to serialize segment");
        fs::write(&file_path, serialized).expect("Failed to write segment to disk");
    }

    /// Load all segments from disk
    pub fn load_all(&mut self) {
        let dir = Path::new(&self.storage_dir);
        if !dir.exists() {
            return;
        }

        for entry in fs::read_dir(dir)
            .expect("Failed to read cold storage directory")
            .flatten()
        {
            if entry.path().extension().is_some_and(|ext| ext == "bin") {
                let data = fs::read(entry.path()).expect("Failed to read segment file");
                let segment: ColdSegment =
                    serde_json::from_slice(&data).expect("Failed to deserialize segment");
                self.segments.push(segment);
            }
        }

        self.segments.sort_by_key(|s| s.start_pos);
    }

    /// Deserialize tokens from a specific segment
    pub fn deserialize_tokens(&self, segment: &ColdSegment) -> Vec<String> {
        serde_json::from_str(&segment.tokens_json)
            .expect("Failed to deserialize tokens from segment")
    }

    /// Get context for a position in cold layer
    pub fn get_context(
        &self,
        global_pos: usize,
        query_len: usize,
        context_window: usize,
    ) -> Option<String> {
        for segment in &self.segments {
            let seg_start = segment.start_pos as usize;
            let seg_end = seg_start + segment.token_count as usize;

            if global_pos >= seg_start && global_pos < seg_end {
                let tokens = self.deserialize_tokens(segment);
                let local_pos = global_pos - seg_start;
                let start = local_pos.saturating_sub(context_window);
                let end = (local_pos + query_len).min(tokens.len());
                if start < end {
                    return Some(tokens[start..end].join(" "));
                }
                return Some(tokens.join(" "));
            }
        }
        None
    }

    /// Search a single cold segment for a phrase
    pub fn segment_phrase_search(segment: &ColdSegment) -> Vec<(u32, String)> {
        let tokens = Self::deserialize_tokens_static(&segment.tokens_json);
        let mut results = Vec::new();
        let seg_start = segment.start_pos;

        if tokens.len() < 2 {
            return results;
        }

        for i in 0..tokens.len().saturating_sub(1) {
            if tokens[i] == tokens[i + 1] {
                results.push((
                    seg_start + i as u32,
                    format!("[Cold] {}-{}", tokens[i], tokens[i + 1]),
                ));
            }
        }

        results
    }

    /// Deserialize tokens from a JSON string (static helper)
    fn deserialize_tokens_static(tokens_json: &str) -> Vec<String> {
        serde_json::from_str(tokens_json).expect("Failed to deserialize tokens from JSON")
    }

    /// Get total size of cold storage in bytes
    pub fn get_storage_size(&self) -> u64 {
        self.segments
            .iter()
            .map(|s| s.tokens_json.len() as u64)
            .sum()
    }
}

/// Hot/Cold split manager
#[derive(Debug)]
pub struct HotColdIndex {
    /// Current hot layer
    pub hot: HotLayer,
    /// Cold layer storage
    pub cold: ColdLayer,
    /// Maximum tokens in hot layer before flush
    pub max_hot_tokens: usize,
    /// Total tokens processed (including those flushed to cold)
    pub total_tokens: u32,
}

impl HotColdIndex {
    pub fn new(storage_dir: &str, max_hot_tokens: usize) -> Self {
        let mut cold = ColdLayer::new(storage_dir);
        cold.load_all();

        let base_offset = if cold.segments.is_empty() {
            0
        } else {
            let last = cold.segments.last().unwrap();
            last.start_pos + last.token_count
        };

        Self {
            hot: HotLayer::new(base_offset),
            cold,
            max_hot_tokens,
            total_tokens: base_offset,
        }
    }

    /// Add token to hot layer, flush if threshold exceeded
    pub fn add_token(&mut self, token: &str) {
        self.hot.add_token(token);
        self.total_tokens += 1;

        if self.hot.tokens.len() >= self.max_hot_tokens {
            self.flush_hot();
        }
    }

    /// Flush hot layer to cold storage
    pub fn flush_hot(&mut self) {
        if self.hot.tokens.is_empty() {
            return;
        }

        self.cold.flush_segment(&self.hot);

        // Create new hot layer with updated base offset
        let new_offset = self.total_tokens;
        self.hot = HotLayer::new(new_offset);
    }

    /// Search across both hot and cold layers
    pub fn phrase_search(&self, query: &[String]) -> Vec<(u32, String)> {
        let mut results = Vec::new();

        if query.is_empty() {
            return results;
        }

        // Search hot layer using bitmap intersection
        let query_first = &query[0];
        if let Some(first_bitmap) = self.hot.token_positions.get(query_first) {
            for pos in first_bitmap.iter() {
                let mut found = true;
                for (j, q_token) in query.iter().enumerate().skip(1) {
                    let target_pos = pos + j as u32;
                    // Check if target position exists in hot layer
                    let hot_start = self.hot.base_offset as usize;
                    let hot_end = hot_start + self.hot.tokens.len();
                    let abs_target = target_pos as usize;

                    if abs_target >= hot_start && abs_target < hot_end {
                        let local_idx = abs_target - hot_start;
                        if local_idx < self.hot.tokens.len() {
                            if self.hot.tokens[local_idx] != *q_token {
                                found = false;
                                break;
                            }
                        } else {
                            found = false;
                            break;
                        }
                    } else {
                        // Check cold layer
                        let token_at_pos = self.get_token_at(abs_target);
                        if token_at_pos.as_deref() != Some(q_token.as_str()) {
                            found = false;
                            break;
                        }
                    }
                }
                if found {
                    let context = self.get_context(pos as usize, query.len());
                    results.push((pos, context));
                }
            }
        }

        // Search cold layer segments (including cross-boundary matches)
        for segment in &self.cold.segments {
            let tokens = self.cold.deserialize_tokens(segment);
            if tokens.is_empty() {
                continue;
            }

            for i in 0..tokens.len() {
                let mut found = true;
                for (j, q_token) in query.iter().enumerate() {
                    let global_idx = segment.start_pos as usize + i + j;
                    let token_at_pos = self.get_token_at(global_idx);
                    if token_at_pos.as_deref() != Some(q_token.as_str()) {
                        found = false;
                        break;
                    }
                }
                if found {
                    let global_pos = segment.start_pos + i as u32;
                    let context = self.get_context(global_pos as usize, query.len());
                    results.push((global_pos, context));
                }
            }
        }

        results.sort_by_key(|(pos, _)| *pos);
        results
    }

    /// Get token at a specific global position
    fn get_token_at(&self, global_pos: usize) -> Option<String> {
        // Check hot layer first
        let hot_start = self.hot.base_offset as usize;
        let hot_end = hot_start + self.hot.tokens.len();

        if global_pos >= hot_start && global_pos < hot_end {
            let local_idx = global_pos - hot_start;
            return Some(self.hot.tokens[local_idx].clone());
        }

        // Check cold layer
        for segment in &self.cold.segments {
            let seg_start = segment.start_pos as usize;
            let seg_end = seg_start + segment.token_count as usize;

            if global_pos >= seg_start && global_pos < seg_end {
                let tokens = self.cold.deserialize_tokens(segment);
                let local_idx = global_pos - seg_start;
                if local_idx < tokens.len() {
                    return Some(tokens[local_idx].clone());
                }
            }
        }

        None
    }

    /// Get context for a global position
    fn get_context(&self, global_pos: usize, query_len: usize) -> String {
        let hot_start = self.hot.base_offset as usize;
        let hot_end = hot_start + self.hot.tokens.len();

        if global_pos >= hot_start && global_pos < hot_end {
            let local_idx = global_pos - hot_start;
            let start = local_idx.saturating_sub(15);
            let end = (local_idx + query_len).min(self.hot.tokens.len());
            if start < end {
                self.hot.tokens[start..end].join(" ")
            } else {
                String::new()
            }
        } else {
            // Cold layer - return segment info
            self.cold
                .get_context(global_pos, query_len, 15)
                .unwrap_or_else(|| "[Cold] Context not available".to_string())
        }
    }

    /// Get statistics
    pub fn stats(&self) -> HotColdStats {
        HotColdStats {
            hot_tokens: self.hot.tokens.len(),
            cold_segments: self.cold.segments.len(),
            cold_storage_bytes: self.cold.get_storage_size(),
            total_tokens: self.total_tokens,
        }
    }
}

/// Statistics for Hot/Cold index
#[derive(Debug)]
pub struct HotColdStats {
    pub hot_tokens: usize,
    pub cold_segments: usize,
    pub cold_storage_bytes: u64,
    pub total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_hot_cold_basic() {
        let temp_dir = env::temp_dir().join("vibe_index_test");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut index = HotColdIndex::new(temp_dir.to_str().unwrap(), 5);

        // Add tokens to hot layer
        for token in ["fn", "main", "(", ")", "{", "let", "mut", "cache"] {
            index.add_token(token);
        }

        assert_eq!(index.hot.tokens.len(), 3); // 8 - 5 (flushed) = 3 remaining
        assert_eq!(index.cold.segments.len(), 1);
        assert_eq!(index.cold.segments[0].token_count, 5);
    }

    #[test]
    fn test_hot_cold_multiple_flushes() {
        let temp_dir = env::temp_dir().join("vibe_index_test2");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut index = HotColdIndex::new(temp_dir.to_str().unwrap(), 3);

        for token in ["a", "b", "c", "d", "e", "f", "g"] {
            index.add_token(token);
        }

        assert_eq!(index.cold.segments.len(), 2); // 7 tokens / 3 = 2 flushes + remaining
    }

    #[test]
    fn test_hot_cold_stats() {
        let temp_dir = env::temp_dir().join("vibe_index_test3");
        let _ = fs::remove_dir_all(&temp_dir);

        let index = HotColdIndex::new(temp_dir.to_str().unwrap(), 100);
        let stats = index.stats();

        assert_eq!(stats.hot_tokens, 0);
        assert_eq!(stats.cold_segments, 0);
        assert_eq!(stats.total_tokens, 0);
    }

    #[test]
    fn test_cold_phrase_search() {
        let temp_dir = env::temp_dir().join("vibe_index_cold_search");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut index = HotColdIndex::new(temp_dir.to_str().unwrap(), 4);

        // Add enough tokens to trigger a flush
        for token in ["fn", "main", "(", ")", "{", "let", "mut", "cache"] {
            index.add_token(token);
        }

        // "fn" and "main" should be in cold layer (positions 0, 1)
        let results = index.phrase_search(&["fn".into(), "main".into()]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0); // Position 0

        // "let" and "mut" should be in hot layer (positions 5, 6)
        let results = index.phrase_search(&["let".into(), "mut".into()]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 5); // Position 5
    }

    #[test]
    fn test_cold_phrase_search_not_found() {
        let temp_dir = env::temp_dir().join("vibe_index_cold_notfound");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut index = HotColdIndex::new(temp_dir.to_str().unwrap(), 4);

        for token in ["fn", "main", "(", ")", "{", "let", "mut", "cache"] {
            index.add_token(token);
        }

        // "not_here" and "found" should not match
        let results = index.phrase_search(&["not_here".into(), "found".into()]);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_cold_context_retrieval() {
        let temp_dir = env::temp_dir().join("vibe_index_cold_context");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut index = HotColdIndex::new(temp_dir.to_str().unwrap(), 4);

        for token in ["fn", "main", "(", ")", "{", "let", "mut", "cache"] {
            index.add_token(token);
        }

        // Get context for position 0 (cold layer)
        let context = index.get_context(0, 2);
        assert!(
            !context.contains("[Cold] Context not available"),
            "Context should be available from cold layer"
        );
    }

    #[test]
    fn test_persist_and_reload_cold_search() {
        let temp_dir = env::temp_dir().join("vibe_index_persist_cold");
        let _ = fs::remove_dir_all(&temp_dir);

        // Create and flush
        {
            let mut index = HotColdIndex::new(temp_dir.to_str().unwrap(), 3);
            for token in ["fn", "main", "(", ")", "{", "let", "mut", "cache"] {
                index.add_token(token);
            }
            // index goes out of scope, cold segments are saved to disk
        }

        // Reload and search cold layer
        let index = HotColdIndex::new(temp_dir.to_str().unwrap(), 100);

        // "fn" and "main" should still be findable in cold layer
        let results = index.phrase_search(&["fn".into(), "main".into()]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0);
    }

    #[test]
    fn test_cold_segment_deserialize_tokens() {
        let temp_dir = env::temp_dir().join("vibe_index_deser");
        let _ = fs::remove_dir_all(&temp_dir);

        let cold = ColdLayer::new(temp_dir.to_str().unwrap());

        let segment = ColdSegment {
            start_pos: 0,
            token_count: 5,
            tokens_json: r#"["fn","main","(","{","}"]"#.to_string(),
        };

        let tokens = cold.deserialize_tokens(&segment);
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0], "fn");
        assert_eq!(tokens[4], "}");
    }

    #[test]
    fn test_cross_layer_phrase_search() {
        let temp_dir = env::temp_dir().join("vibe_index_cross");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut index = HotColdIndex::new(temp_dir.to_str().unwrap(), 4);

        // Tokens: fn(0) main(1) ( (2) ) (3) { (4) let(5) mut(6) cache(7)
        // After flush at 4: cold = [fn, main, (, )], hot = [{, let, mut, cache]
        for token in ["fn", "main", "(", ")", "{", "let", "mut", "cache"] {
            index.add_token(token);
        }

        // Search across the boundary: ")" is last in cold, "{" is first in hot
        // They are consecutive: position 3 (cold) and position 4 (hot)
        let results = index.phrase_search(&[")".into(), "{".into()]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 3); // ")" is at position 3
    }
}
