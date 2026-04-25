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
    /// Compressed token data (gzip)
    pub compressed_tokens: Vec<u8>,
    /// Delta-encoded position bitmaps
    pub compressed_bitmaps: Vec<u8>,
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
            .or_insert_with(RoaringBitmap::new)
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

        // Compress tokens using simple run-length encoding for demo
        // In production, use zstd or lz4
        let token_bytes: Vec<u8> = hot_layer.tokens.iter()
            .flat_map(|t| t.as_bytes().to_vec())
            .collect();

        // Simple gzip-like compression (in production, use flate2)
        let compressed_tokens = token_bytes; // Placeholder - would use flate2::write::GzEncoder

        // Compress position bitmaps using delta encoding
        let mut compressed_bitmaps = Vec::new();
        for (_token, bitmap) in &hot_layer.token_positions {
            let mut last_pos: u32 = 0;
            for pos in bitmap.iter() {
                let delta = pos - last_pos;
                // Variable-byte encoding for delta
                let mut v = delta;
                while v >= 0x80 {
                    compressed_bitmaps.push((v & 0x7F) as u8 | 0x80);
                    v >>= 7;
                }
                compressed_bitmaps.push(v as u8);
                last_pos = pos;
            }
        }

        let segment = ColdSegment {
            start_pos: hot_layer.base_offset,
            token_count: hot_layer.tokens.len() as u32,
            compressed_tokens,
            compressed_bitmaps,
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

        for entry in fs::read_dir(dir).expect("Failed to read cold storage directory") {
            if let Ok(entry) = entry {
                if entry.path().extension().map_or(false, |ext| ext == "bin") {
                    let data = fs::read(entry.path()).expect("Failed to read segment file");
                    let segment: ColdSegment = serde_json::from_slice(&data)
                        .expect("Failed to deserialize segment");
                    self.segments.push(segment);
                }
            }
        }

        self.segments.sort_by_key(|s| s.start_pos);
    }

    /// Get context for a position in cold layer
    pub fn get_context(&self, global_pos: usize, _query_len: usize) -> Option<String> {
        for segment in &self.segments {
            let seg_start = segment.start_pos as usize;
            let seg_end = seg_start + segment.token_count as usize;

            if global_pos >= seg_start && global_pos < seg_end {
                // In production, decompress and extract context
                // For now, return placeholder
                return Some(format!("[Cold] Segment {}-{}", seg_start, seg_end));
            }
        }
        None
    }

    /// Get total size of cold storage in bytes
    pub fn get_storage_size(&self) -> u64 {
        self.segments.iter().map(|s| {
            s.compressed_tokens.len() as u64 + s.compressed_bitmaps.len() as u64
        }).sum()
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

        // Search hot layer
        for (_i, token) in query.iter().enumerate() {
            if let Some(bitmap) = self.hot.token_positions.get(token) {
                for pos in bitmap.iter() {
                    // Check if all query tokens are at consecutive positions
                    let mut found = true;
                    for (j, q_token) in query.iter().enumerate().skip(1) {
                        let _target_pos = pos + j as u32;
                        let token_at_pos = self.get_token_at(pos as usize + j);
                        if token_at_pos.as_deref() != Some(q_token.as_str()) {
                            found = false;
                            break;
                        }
                    }
                    if found {
                        let context = self.get_context(pos as usize, query.len());
                        results.push((pos, context));
                    }
                }
            }
        }

        // Search cold layer (simplified - in production, would use bitmap intersection)
        for segment in &self.cold.segments {
            let _seg_start = segment.start_pos as usize;
            // In production, decompress bitmaps and perform search
            // For now, mark cold search as needing implementation
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
        self.cold.get_context(global_pos, 1)
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
            self.cold.get_context(global_pos, query_len).unwrap_or_else(|| "[Cold] Context not available".to_string())
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
}
