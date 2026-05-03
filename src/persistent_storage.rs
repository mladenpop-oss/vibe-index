use crate::file_index::FileIndex;
use crate::VibeIndex;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

/// Magic bytes for file format validation
const MAGIC: &[u8] = b"VIBE";
const VERSION: u32 = 4;
const LEGACY_V3: u32 = 3;
const LEGACY_V2: u32 = 2;
const LEGACY_V1: u32 = 1;

/// Persistent storage format for VibeIndex
#[derive(Debug, Serialize, Deserialize)]
pub struct PersistentIndex {
    /// File format version
    version: u32,
    /// Total number of tokens indexed
    total_tokens: u32,
    /// Token sequence (gzip compressed)
    #[serde(skip_serializing_if = "Option::is_none")]
    compressed_tokens: Option<Vec<u8>>,
    /// Token lexicon: list of unique token strings (index = token ID)
    #[serde(skip_serializing_if = "Option::is_none")]
    lexicon: Option<Vec<String>>,
    /// Serialized token position bitmaps (token_id -> base64-encoded roaring bitmap)
    #[serde(skip_serializing_if = "Option::is_none")]
    token_positions: Option<HashMap<String, String>>,
    /// File metadata: file paths, content, line offsets, token-to-line mappings
    #[serde(skip_serializing_if = "Option::is_none")]
    file_index: Option<FileIndex>,
}

/// Persistent storage manager for VibeIndex
pub struct PersistentStorage {
    /// Path to the index file
    index_path: String,
    /// Current index in memory
    index: VibeIndex,
    /// Whether index has been modified since last save
    dirty: bool,
}

impl PersistentStorage {
    /// Create new persistent storage
    pub fn new(index_path: &str) -> Self {
        let path = Path::new(index_path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).ok();
            }
        }

        Self {
            index_path: index_path.to_string(),
            index: VibeIndex::new(),
            dirty: false,
        }
    }

    /// Load existing index from disk, or create new if not found
    pub fn load(index_path: &str) -> Self {
        let mut storage = Self::new(index_path);

        if Path::new(&storage.index_path).exists() {
            match storage.load_from_disk() {
                Ok(_) => {
                    eprintln!(
                        "✅ Loaded {} tokens from {}",
                        storage.index.total_positions(),
                        storage.index_path
                    );
                }
                Err(e) => {
                    eprintln!("⚠️ Failed to load index: {}. Starting fresh.", e);
                }
            }
        }

        storage
    }

    /// Add token to the index
    pub fn add_token(&mut self, token: &str) {
        self.index.add_token(token);
        self.dirty = true;
    }

    /// Add a file with path metadata
    pub fn add_file(&mut self, path: &str, content: &str) {
        self.index.add_file(path, content);
        self.dirty = true;
    }

    /// Search for a phrase
    pub fn phrase_search(&self, query: &[String]) -> Vec<crate::MatchResult> {
        self.index.phrase_search(query)
    }

    /// Search using unified natural language query
    pub fn search(&self, query: &str) -> Vec<crate::MatchResult> {
        self.index.search(query)
    }

    /// Get total token count
    pub fn total_tokens(&self) -> usize {
        self.index.total_positions()
    }

    /// Get unique token count
    pub fn unique_tokens(&self) -> usize {
        self.index.unique_tokens()
    }

    /// Get total file count
    pub fn total_files(&self) -> usize {
        self.index.file_index.files.len()
    }

    /// Save index to disk (synchronous)
    pub fn save(&mut self) -> Result<(), anyhow::Error> {
        if !self.dirty {
            return Ok(());
        }

        Self::save_index(&self.index, &self.index_path)?;
        self.dirty = false;
        eprintln!(
            "💾 Saved {} tokens to {}",
            self.index.total_positions(),
            self.index_path
        );
        Ok(())
    }

    /// Load index from disk
    fn load_from_disk(&mut self) -> Result<(), anyhow::Error> {
        let data = fs::read(&self.index_path)?;

        // Validate magic bytes
        if &data[..4] != MAGIC {
            return Err(anyhow::anyhow!("Invalid index file format"));
        }

        // Validate version
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if version != VERSION
            && version != LEGACY_V3
            && version != LEGACY_V2
            && version != LEGACY_V1
        {
            return Err(anyhow::anyhow!(
                "Incompatible index version: expected {}, {}, {}, or {}, got {}",
                VERSION,
                LEGACY_V3,
                LEGACY_V2,
                LEGACY_V1,
                version
            ));
        }

        // Deserialize the persistent index
        let serialized_len = 8; // magic + version
        let persistent: PersistentIndex = serde_json::from_slice(&data[serialized_len..])?;

        if version == VERSION {
            // v4: lexicon-based format with u32 token IDs
            let lexicon: crate::TokenLexicon = if let Some(ref lex_entries) = persistent.lexicon {
                let mut lex = crate::TokenLexicon::new();
                for token in lex_entries {
                    lex.get_or_insert(token);
                }
                lex
            } else {
                crate::TokenLexicon::new()
            };

            let token_sequence: Vec<u32> = if let Some(ref compressed) =
                persistent.compressed_tokens
            {
                let mut decoder = GzDecoder::new(compressed.as_slice());
                let mut token_bytes = Vec::new();
                decoder.read_to_end(&mut token_bytes)?;
                bincode::deserialize(&token_bytes)
                    .map_err(|e| anyhow::anyhow!("Failed to deserialize token sequence: {}", e))?
            } else {
                Vec::new()
            };

            let token_positions: HashMap<u32, RoaringBitmap> =
                if let Some(ref positions_map) = persistent.token_positions {
                    let mut result = HashMap::new();
                    for (id_str, encoded) in positions_map {
                        let id: u32 = id_str
                            .parse()
                            .map_err(|e| anyhow::anyhow!("Invalid token ID '{}': {}", id_str, e))?;
                        let buf = STANDARD.decode(encoded).map_err(|e| {
                            anyhow::anyhow!("Failed to decode bitmap for ID {}: {}", id, e)
                        })?;
                        let bitmap = RoaringBitmap::deserialize_from(&buf[..]).map_err(|e| {
                            anyhow::anyhow!("Failed to deserialize bitmap for ID {}: {}", id, e)
                        })?;
                        result.insert(id, bitmap);
                    }
                    result
                } else {
                    HashMap::new()
                };

            let file_index = persistent.file_index.clone().unwrap_or_default();
            self.index = VibeIndex::from_persistent(
                lexicon,
                token_positions,
                token_sequence,
                persistent.total_tokens as usize,
                file_index,
            );
        } else {
            // v1/v2/v3: legacy string-based format
            let mut decoder =
                GzDecoder::new(persistent.compressed_tokens.as_ref().unwrap().as_slice());
            let mut token_bytes = Vec::new();
            decoder.read_to_end(&mut token_bytes)?;
            let token_str = String::from_utf8(token_bytes)?;
            let tokens: Vec<String> = token_str
                .split('\n')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();

            if let Some(ref positions_map) = persistent.token_positions {
                let mut token_positions: HashMap<String, RoaringBitmap> = HashMap::new();
                for (token, encoded) in positions_map {
                    let bitmap = if version == LEGACY_V3 {
                        let buf = STANDARD.decode(encoded).map_err(|e| {
                            anyhow::anyhow!("Failed to decode bitmap for '{}': {}", token, e)
                        })?;
                        RoaringBitmap::deserialize_from(&buf[..]).map_err(|e| {
                            anyhow::anyhow!("Failed to deserialize bitmap for '{}': {}", token, e)
                        })?
                    } else {
                        let positions: Vec<u32> = serde_json::from_str(encoded).map_err(|e| {
                            anyhow::anyhow!(
                                "Failed to deserialize positions for '{}': {}",
                                token,
                                e
                            )
                        })?;
                        let mut bitmap = RoaringBitmap::new();
                        for pos in positions {
                            bitmap.push(pos);
                        }
                        bitmap
                    };
                    token_positions.insert(token.clone(), bitmap);
                }
                self.index = VibeIndex::from_legacy(
                    token_positions,
                    tokens,
                    persistent.total_tokens as usize,
                );
            } else {
                for token in &tokens {
                    self.index.add_token(token);
                }
            }
        }

        Ok(())
    }

    /// Save index to disk
    fn save_index(index: &VibeIndex, path: &str) -> Result<(), anyhow::Error> {
        // Serialize token sequence as bincode Vec<u32>
        let token_bytes = bincode::serialize(&index.token_sequence)
            .map_err(|e| anyhow::anyhow!("Failed to serialize token sequence: {}", e))?;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&token_bytes)?;
        let compressed_tokens = encoder.finish()?;

        // Serialize lexicon
        let lexicon: Vec<String> = index.lexicon.iter().map(|(_, t)| t.to_string()).collect();

        // Serialize token position bitmaps keyed by u32 ID string
        let mut positions_map: HashMap<String, String> = HashMap::new();
        for (&token_id, bitmap) in &index.token_positions {
            let mut buf = Vec::with_capacity(bitmap.serialized_size());
            bitmap.serialize_into(&mut buf).map_err(|e| {
                anyhow::anyhow!("Failed to serialize bitmap for ID {}: {}", token_id, e)
            })?;
            let encoded = STANDARD.encode(&buf);
            positions_map.insert(token_id.to_string(), encoded);
        }

        let persistent = PersistentIndex {
            version: VERSION,
            total_tokens: index.total_positions() as u32,
            compressed_tokens: Some(compressed_tokens),
            lexicon: Some(lexicon),
            token_positions: Some(positions_map),
            file_index: Some(index.file_index.clone()),
        };

        let json = serde_json::to_vec(&persistent)?;

        let mut data = Vec::new();
        data.extend_from_slice(MAGIC);
        data.extend_from_slice(&VERSION.to_le_bytes());
        data.extend_from_slice(&json);

        fs::write(path, data)?;

        Ok(())
    }

    /// Compact the index (rebuild from bitmaps to save space)
    pub fn compact(&mut self) -> Result<(), anyhow::Error> {
        // In production, this would rebuild the index from bitmaps
        // For now, just trigger a save
        self.dirty = true;
        self.save()
    }

    /// Get storage size in bytes
    pub fn get_storage_size(&self) -> Result<u64, anyhow::Error> {
        Ok(fs::metadata(&self.index_path)?.len())
    }

    /// Delete the index file
    pub fn delete(&self) -> Result<(), anyhow::Error> {
        if Path::new(&self.index_path).exists() {
            fs::remove_file(&self.index_path)?;
        }
        Ok(())
    }

    /// Get statistics
    pub fn stats(&self) -> StorageStats {
        StorageStats {
            total_tokens: self.index.total_positions(),
            unique_tokens: self.index.unique_tokens(),
            storage_size: self.get_storage_size().unwrap_or(0),
            is_dirty: self.dirty,
        }
    }
}

/// Statistics for persistent storage
#[derive(Debug)]
pub struct StorageStats {
    pub total_tokens: usize,
    pub unique_tokens: usize,
    pub storage_size: u64,
    pub is_dirty: bool,
}

/// Batch import from a list of tokens
pub fn batch_import(index_path: &str, tokens: Vec<&str>) -> Result<(), anyhow::Error> {
    let mut storage = PersistentStorage::new(index_path);

    for token in tokens {
        storage.add_token(token);
    }

    storage.save()?;
    Ok(())
}

/// Batch import from a text file (tokenized by whitespace)
pub fn import_from_file(index_path: &str, file_path: &str) -> Result<(), anyhow::Error> {
    let content = fs::read_to_string(file_path)?;
    let tokens: Vec<&str> = content.split_whitespace().collect();
    batch_import(index_path, tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_save_and_load() {
        let temp_dir = env::temp_dir().join("vibe_index_persistent");
        let _ = fs::remove_dir_all(&temp_dir);
        let binding = temp_dir.join("index.bin");
        let index_path = binding.to_str().unwrap();

        let mut storage = PersistentStorage::new(index_path);

        // Add tokens
        for token in ["fn", "main", "(", ")", "{", "let", "x", "=", "42", ";"] {
            storage.add_token(token);
        }

        assert_eq!(storage.total_tokens(), 10);

        // Save
        storage.save().unwrap();

        // Load
        let loaded = PersistentStorage::load(index_path);
        assert_eq!(loaded.total_tokens(), 10);

        // Verify search still works
        let results = loaded.phrase_search(&["fn".into(), "main".into()]);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_persistence_across_instances() {
        let temp_dir = env::temp_dir().join("vibe_index_persist2");
        let _ = fs::remove_dir_all(&temp_dir);
        let binding = temp_dir.join("index2.bin");
        let index_path = binding.to_str().unwrap();

        // Create and save
        {
            let mut storage = PersistentStorage::new(index_path);
            for token in ["hello", "world", "test", "data"] {
                storage.add_token(token);
            }
            storage.save().unwrap();
            drop(storage); // Explicitly drop
        }

        // Load in new instance
        let mut loaded = PersistentStorage::load(index_path);
        assert_eq!(loaded.total_tokens(), 4);

        // Add more tokens
        loaded.add_token("more");
        loaded.add_token("data");
        loaded.save().unwrap();

        // Verify both old and new tokens
        let loaded2 = PersistentStorage::load(index_path);
        assert_eq!(loaded2.total_tokens(), 6);
    }

    #[test]
    fn test_batch_import() {
        let temp_dir = env::temp_dir().join("vibe_index_batch");
        let _ = fs::remove_dir_all(&temp_dir);
        let binding = temp_dir.join("batch.bin");
        let index_path = binding.to_str().unwrap();

        let tokens = vec![
            "fn", "add", "(", "a", ":", "i32", ")", "→", "i32", "{", "a", "+", "1", "}",
        ];
        batch_import(index_path, tokens).unwrap();

        let loaded = PersistentStorage::load(index_path);
        assert_eq!(loaded.total_tokens(), 14);
    }

    #[test]
    fn test_storage_stats() {
        let temp_dir = env::temp_dir().join("vibe_index_stats");
        let _ = fs::remove_dir_all(&temp_dir);
        let binding = temp_dir.join("stats.bin");
        let index_path = binding.to_str().unwrap();

        let mut storage = PersistentStorage::new(index_path);
        for token in ["test", "data", "here"] {
            storage.add_token(token);
        }

        let stats = storage.stats();
        assert_eq!(stats.total_tokens, 3);
        assert_eq!(stats.unique_tokens, 3);
        assert!(stats.is_dirty);

        storage.save().unwrap();
        let stats = storage.stats();
        assert!(stats.storage_size > 0);
        assert!(!stats.is_dirty);
    }

    #[test]
    fn test_invalid_file() {
        let temp_dir = env::temp_dir().join("vibe_index_invalid");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).ok();
        let binding = temp_dir.join("invalid.bin");
        let index_path = binding.to_str().unwrap();

        // Create invalid file
        fs::write(index_path, b"INVALID FORMAT").unwrap();

        // Loading should fail gracefully
        let storage = PersistentStorage::load(index_path);
        assert_eq!(storage.total_tokens(), 0);
    }

    #[test]
    fn test_bitmaps_loaded_from_disk() {
        let temp_dir = env::temp_dir().join("vibe_index_bitmap_load");
        let _ = fs::remove_dir_all(&temp_dir);
        let binding = temp_dir.join("bitmap.bin");
        let index_path = binding.to_str().unwrap();

        // Create and save index
        {
            let mut storage = PersistentStorage::new(index_path);
            for token in ["fn", "main", "(", ")", "{", "fn", "main", "("] {
                storage.add_token(token);
            }
            storage.save().unwrap();
        }

        // Load and verify bitmaps are restored
        let loaded = PersistentStorage::load(index_path);
        assert_eq!(loaded.total_tokens(), 8);

        // Verify bitmap data is correct (fn appears at positions 0 and 5)
        let fn_id = loaded.index.lexicon.get_id("fn").unwrap();
        let fn_bitmap = loaded.index.token_positions.get(&fn_id).unwrap();
        assert_eq!(fn_bitmap.len(), 2);
        assert!(fn_bitmap.contains(0));
        assert!(fn_bitmap.contains(5));

        // Verify phrase search works with loaded bitmaps
        let results = loaded.phrase_search(&["fn".into(), "main".into()]);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].position, 0);
        assert_eq!(results[1].position, 5);
    }

    #[test]
    fn test_file_index_persistence() {
        let temp_dir = env::temp_dir().join("vibe_index_file_persist");
        let _ = fs::remove_dir_all(&temp_dir);
        let binding = temp_dir.join("file_index.bin");
        let index_path = binding.to_str().unwrap();

        // Create and save with file metadata
        {
            let mut storage = PersistentStorage::new(index_path);
            storage.add_file(
                "src/auth.rs",
                "fn authenticate(user: &str) -> Result<(), Error> {\n    Ok(())\n}\n",
            );
            storage.add_file("src/main.rs", "fn main() {\n    let x = 42;\n}\n");
            storage.save().unwrap();
        }

        // Load and verify file metadata is restored
        let loaded = PersistentStorage::load(index_path);
        assert_eq!(loaded.index.file_index.files.len(), 2);
        assert_eq!(loaded.index.file_index.files[0].path, "src/auth.rs");
        assert_eq!(loaded.index.file_index.files[1].path, "src/main.rs");

        // Verify file info is available in search results
        let results = loaded.phrase_search(&["fn".into(), "authenticate".into()]);
        assert_eq!(results.len(), 1);
        assert!(results[0].file_path.is_some());
        assert_eq!(results[0].file_path.as_deref(), Some("src/auth.rs"));
        assert!(results[0].line_number.is_some());
        assert_eq!(results[0].line_number.unwrap(), 1);
        assert!(results[0].line_content.is_some());
        assert!(results[0]
            .line_content
            .as_deref()
            .unwrap()
            .contains("authenticate"));
    }

    #[test]
    fn test_legacy_load_without_file_index() {
        let temp_dir = env::temp_dir().join("vibe_index_legacy_no_file");
        let _ = fs::remove_dir_all(&temp_dir);
        let binding = temp_dir.join("legacy.bin");
        let index_path = binding.to_str().unwrap();

        // Create and save WITHOUT file metadata (simulate old format)
        {
            let mut storage = PersistentStorage::new(index_path);
            for token in ["hello", "world", "test"] {
                storage.add_token(token);
            }
            storage.save().unwrap();
        }

        // Load should still work (file_index will be empty)
        let loaded = PersistentStorage::load(index_path);
        assert_eq!(loaded.total_tokens(), 3);
        assert_eq!(loaded.index.file_index.files.len(), 0);

        // Search should still work
        let results = loaded.search("hello world");
        assert!(!results.is_empty());
    }
}
