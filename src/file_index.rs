use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Represents a single file that was indexed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSegment {
    /// File path (relative or absolute)
    pub path: String,
    /// Original file content (for context extraction)
    pub content: String,
    /// Token start position in the global token stream
    pub token_start: usize,
    /// Token end position in the global token stream (exclusive)
    pub token_end: usize,
    /// Line number offsets in bytes (line_offsets[i] = byte offset of line i+1)
    pub line_offsets: Vec<usize>,
    /// Direct mapping: token_line_map[i] = 1-indexed line number for token at local position i
    pub token_line_map: Vec<usize>,
    /// Number of tokens in this file
    pub token_count: usize,
}

impl FileSegment {
    pub fn new(path: String, content: String, token_start: usize, token_end: usize) -> Self {
        let line_offsets = Self::compute_line_offsets(&content);
        let token_line_map = Self::build_token_line_map(&content);
        Self {
            path,
            content,
            token_start,
            token_end,
            line_offsets,
            token_line_map,
            token_count: token_end - token_start,
        }
    }

    /// Compute byte offsets for each line in the file content
    pub fn compute_line_offsets(content: &str) -> Vec<usize> {
        let mut offsets = Vec::new();
        let mut current = 0;
        offsets.push(current);
        for byte in content.bytes() {
            if byte == b'\n' {
                current += 1;
                offsets.push(current);
            } else {
                current += 1;
            }
        }
        offsets
    }

    /// Build a direct mapping from local token position to 1-indexed line number.
    /// Uses the same tokenization as VibeIndex: split on non-alphanumeric, filter empty.
    pub fn build_token_line_map(content: &str) -> Vec<usize> {
        let mut line_map = Vec::new();
        let mut current_line = 1usize;
        let mut chars = content.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\n' {
                current_line += 1;
                continue;
            }

            // Check if this character starts a token (is alphanumeric or underscore)
            if ch.is_alphanumeric() || ch == '_' {
                line_map.push(current_line);
                // Skip the rest of this token
                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_alphanumeric() || next_ch == '_' {
                        chars.next();
                    } else {
                        break;
                    }
                }
            }
        }

        line_map
    }

    /// Convert a global token position to a (file_line, line_context) tuple.
    /// Uses precomputed direct mapping — O(1) lookup.
    pub fn token_to_line(&self, global_token_pos: usize) -> Option<(usize, String)> {
        if global_token_pos < self.token_start || global_token_pos >= self.token_end {
            return None;
        }

        let local_token_pos = global_token_pos - self.token_start;
        let line_number = *self.token_line_map.get(local_token_pos)?;
        let line_content = self.get_line_content(line_number);

        Some((line_number, line_content))
    }

    /// Get the content of a specific line (1-indexed)
    pub fn get_line_content(&self, line_number: usize) -> String {
        if line_number == 0 || line_number > self.line_offsets.len() {
            return String::new();
        }

        let start = self.line_offsets[line_number - 1];
        let end = if line_number < self.line_offsets.len() {
            self.line_offsets[line_number]
        } else {
            self.content.len()
        };

        self.content[start..end].trim_end_matches('\n').to_string()
    }

    /// Check if a global token position is within this file's range
    pub fn contains_token(&self, global_pos: usize) -> bool {
        global_pos >= self.token_start && global_pos < self.token_end
    }
}

/// File-aware index manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileIndex {
    /// All indexed files (kept in insertion order)
    pub files: Vec<FileSegment>,
}

impl Default for FileIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl FileIndex {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// Add a file to the index
    pub fn add_file(
        &mut self,
        path: String,
        content: String,
        token_start: usize,
        token_end: usize,
    ) {
        let file = FileSegment::new(path, content, token_start, token_end);
        self.files.push(file);
    }

    /// Build a sorted index for binary search (call after all files added)
    pub fn build_lookup_index(&mut self) {
        self.files.sort_by_key(|f| f.token_start);
    }

    /// Get file path for a token position using binary search
    pub fn get_file_path(&self, token_pos: usize) -> Option<&str> {
        let idx = self.files.binary_search_by(|f| {
            if token_pos < f.token_start {
                Ordering::Greater
            } else if token_pos >= f.token_end {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        });
        idx.ok().map(|i| self.files[i].path.as_str())
    }

    /// Get file info for a token position using binary search
    pub fn get_file_info(&self, token_pos: usize) -> Option<(usize, String, String)> {
        let idx = self.files.binary_search_by(|f| {
            if token_pos < f.token_start {
                Ordering::Greater
            } else if token_pos >= f.token_end {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        });
        let i = idx.ok()?;
        let file = &self.files[i];
        if let Some((_, line_content)) = file.token_to_line(token_pos) {
            Some((i, file.path.clone(), line_content))
        } else {
            None
        }
    }

    /// Get all files that contain tokens matching a position range
    pub fn get_files_in_range(&self, start: usize, end: usize) -> Vec<&FileSegment> {
        self.files
            .iter()
            .filter(|f| f.token_start < end && f.token_end > start)
            .collect()
    }

    /// Get file statistics
    pub fn stats(&self) -> FileIndexStats {
        FileIndexStats {
            total_files: self.files.len(),
            total_tokens: self.files.iter().map(|f| f.token_count).sum(),
            files: self
                .files
                .iter()
                .map(|f| (f.path.clone(), f.token_count))
                .collect(),
        }
    }
}

/// Statistics for file index
#[derive(Debug)]
pub struct FileIndexStats {
    pub total_files: usize,
    pub total_tokens: usize,
    pub files: Vec<(String, usize)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_segment_creation() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let segment = FileSegment::new("test.rs".to_string(), content.to_string(), 0, 10);
        assert_eq!(segment.path, "test.rs");
        assert_eq!(segment.token_start, 0);
        assert_eq!(segment.token_end, 10);
        assert!(!segment.line_offsets.is_empty());
        assert!(!segment.token_line_map.is_empty());
    }

    #[test]
    fn test_line_offset_computation() {
        let content = "line1\nline2\nline3\n";
        let offsets = FileSegment::compute_line_offsets(content);
        assert_eq!(offsets.len(), 4);
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[1], 6);
        assert_eq!(offsets[2], 12);
    }

    #[test]
    fn test_token_line_map_accuracy() {
        let content = "fn main() {\n    let x = 42;\n}\n";
        let tokens: Vec<&str> = content
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect();
        // Tokens (split-based): fn, main, let, x, 42 = 5 tokens
        // {, }, (, ), ;, = are delimiters (non-alphanumeric)
        // Line 1: fn, main      -> tokens 0-1
        // Line 2: let, x, 42    -> tokens 2-4
        let map = FileSegment::build_token_line_map(content);
        assert_eq!(map.len(), tokens.len());
        assert_eq!(map[0], 1); // fn -> line 1
        assert_eq!(map[1], 1); // main -> line 1
        assert_eq!(map[2], 2); // let -> line 2
        assert_eq!(map[3], 2); // x -> line 2
        assert_eq!(map[4], 2); // 42 -> line 2
    }

    #[test]
    fn test_token_to_line_exact() {
        let content = "fn authenticate(user: &str) -> Result<(), Error> {\n    Ok(())\n}\n";
        let token_count: usize = content
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        let segment = FileSegment::new(
            "src/auth.rs".to_string(),
            content.to_string(),
            0,
            token_count,
        );

        // "fn" is token 0, should be on line 1
        let (line, lc) = segment.token_to_line(0).unwrap();
        assert_eq!(line, 1);
        assert!(lc.contains("authenticate"));

        // "Ok" is the last token, should be on line 2
        let (line, lc) = segment.token_to_line(token_count - 1).unwrap();
        assert_eq!(line, 2);
        assert!(lc.contains("Ok"));
    }

    #[test]
    fn test_get_line_content() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let token_count: usize = content
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        let segment = FileSegment::new("test.rs".to_string(), content.to_string(), 0, token_count);
        assert_eq!(segment.get_line_content(1), "fn main() {");
        assert_eq!(segment.get_line_content(2), "    println!(\"hello\");");
        assert_eq!(segment.get_line_content(3), "}");
    }

    #[test]
    fn test_token_to_line() {
        let content = "fn main() {\n    let x = 42;\n}\n";
        let token_count: usize = content
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        let segment = FileSegment::new("test.rs".to_string(), content.to_string(), 0, token_count);
        // Token 2 is "let" on line 2
        let result = segment.token_to_line(2);
        assert!(result.is_some());
        let (line_num, line_content) = result.unwrap();
        assert_eq!(line_num, 2);
        assert!(line_content.contains("let"));
        assert!(line_content.contains("42"));
    }

    #[test]
    fn test_file_index_stats() {
        let mut index = FileIndex::new();
        let t1 = "fn main() {}"
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        let t2 = "fn main() {}"
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        index.add_file("src/lib.rs".to_string(), "fn main() {}".to_string(), 0, t1);
        index.add_file(
            "src/main.rs".to_string(),
            "fn main() {}".to_string(),
            t1,
            t1 + t2,
        );
        let stats = index.stats();
        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.total_tokens, t1 + t2);
    }

    #[test]
    fn test_get_file_path() {
        let mut index = FileIndex::new();
        let t1 = "fn a() {}"
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        let t2 = "fn b() {}"
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        index.add_file("src/a.rs".to_string(), "fn a() {}".to_string(), 0, t1);
        index.add_file("src/b.rs".to_string(), "fn b() {}".to_string(), t1, t1 + t2);

        assert_eq!(index.get_file_path(0), Some("src/a.rs"));
        assert_eq!(index.get_file_path(t1), Some("src/b.rs"));
        assert_eq!(index.get_file_path(999), None);
    }

    #[test]
    fn test_get_file_info() {
        let mut index = FileIndex::new();
        let content = "fn hello() {}\nfn world() {}\n";
        let token_count = content
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        index.add_file("test.rs".to_string(), content.to_string(), 0, token_count);

        let info = index.get_file_info(0);
        assert!(info.is_some());
        let (idx, path, line_content) = info.unwrap();
        assert_eq!(idx, 0);
        assert_eq!(path, "test.rs");
        assert!(line_content.contains("hello"));
    }

    #[test]
    fn test_binary_search_single_file() {
        let mut index = FileIndex::new();
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let token_count = content
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        index.add_file("test.rs".to_string(), content.to_string(), 0, token_count);
        index.build_lookup_index();

        assert_eq!(index.get_file_path(0), Some("test.rs"));
        assert_eq!(index.get_file_path(token_count / 2), Some("test.rs"));
        assert_eq!(index.get_file_path(token_count - 1), Some("test.rs"));
        assert_eq!(index.get_file_path(token_count), None);
        assert_eq!(index.get_file_path(999), None);
    }

    #[test]
    fn test_binary_search_multiple_files() {
        let mut index = FileIndex::new();
        let t1 = "fn a() {}"
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        let t2 = "fn b() {}"
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        let t3 = "fn c() {}"
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();

        index.add_file("src/b.rs".to_string(), "fn b() {}".to_string(), t1, t1 + t2);
        index.add_file("src/a.rs".to_string(), "fn a() {}".to_string(), 0, t1);
        index.add_file(
            "src/c.rs".to_string(),
            "fn c() {}".to_string(),
            t1 + t2,
            t1 + t2 + t3,
        );
        index.build_lookup_index();

        assert_eq!(index.get_file_path(0), Some("src/a.rs"));
        assert_eq!(index.get_file_path(t1), Some("src/b.rs"));
        assert_eq!(index.get_file_path(t1 + t2), Some("src/c.rs"));
        assert_eq!(index.get_file_path(999), None);
    }

    #[test]
    fn test_binary_search_not_found() {
        let mut index = FileIndex::new();
        index.add_file("src/a.rs".to_string(), "fn a() {}".to_string(), 0, 4);
        index.add_file("src/b.rs".to_string(), "fn b() {}".to_string(), 10, 14);
        index.build_lookup_index();

        assert_eq!(index.get_file_path(5), None);
        assert_eq!(index.get_file_path(8), None);
        assert_eq!(index.get_file_path(20), None);
    }

    #[test]
    fn test_binary_search_boundary() {
        let mut index = FileIndex::new();
        let t1 = "fn a() {}"
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        let t2 = "fn b() {}"
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        index.add_file("src/a.rs".to_string(), "fn a() {}".to_string(), 0, t1);
        index.add_file("src/b.rs".to_string(), "fn b() {}".to_string(), t1, t1 + t2);
        index.build_lookup_index();

        assert_eq!(index.get_file_path(t1 - 1), Some("src/a.rs"));
        assert_eq!(index.get_file_path(t1), Some("src/b.rs"));
    }

    #[test]
    fn test_get_file_info_binary_search() {
        let mut index = FileIndex::new();
        let content1 = "fn hello() {}\nfn world() {}\n";
        let content2 = "struct Foo {}\nimpl Bar {}\n";
        let t1 = content1
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();
        let t2 = content2
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .count();

        index.add_file("src/b.rs".to_string(), content2.to_string(), t1, t1 + t2);
        index.add_file("src/a.rs".to_string(), content1.to_string(), 0, t1);
        index.build_lookup_index();

        let info = index.get_file_info(0);
        assert!(info.is_some());
        let (_, path, line_content) = info.unwrap();
        assert_eq!(path, "src/a.rs");
        assert!(line_content.contains("hello"));
    }
}
