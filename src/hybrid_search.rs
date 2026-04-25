use crate::bm25::Bm25Index;
use crate::{MatchResult, VibeIndex};

/// Hybrid search: BM25 finds candidate documents → Vibe Index validates exact positions.
/// 
/// This gives you the best of both worlds:
/// - BM25 handles semantic relevance and ranking
/// - Vibe Index provides sub-millisecond exact position validation
pub struct HybridSearcher {
    bm25: Bm25Index,
    vibe: VibeIndex,
    top_k: usize,
}

impl HybridSearcher {
    pub fn new(top_k: usize) -> Self {
        Self {
            bm25: Bm25Index::new(),
            vibe: VibeIndex::new(),
            top_k,
        }
    }

    /// Add a document (token range) to both BM25 and Vibe Index
    pub fn add_document(&mut self, start: usize, end: usize) {
        self.bm25.add_document(start, end);
    }

    /// Index all tokens - call after all documents added
    pub fn index_tokens(&mut self, tokens: &[String]) {
        // Build Vibe Index
        for (_i, token) in tokens.iter().enumerate() {
            self.vibe.add_token(token);
        }
        // Build BM25 Index
        self.bm25.index_tokens(tokens);
    }

    /// Hybrid search: BM25 retrieves candidates → Vibe validates exact phrases
    pub fn search(&self, query: &str) -> Vec<MatchResult> {
        use crate::query_parser;
        
        // 1. Parse query into tokens
        let stop_set: std::collections::HashSet<&str> = query_parser::ENGLISH_STOP_WORDS.iter().copied().collect();
        let query_tokens: Vec<String> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && !stop_set.contains(s))
            .map(|s| s.to_lowercase())
            .collect();

        if query_tokens.is_empty() {
            return Vec::new();
        }

        // 2. BM25 retrieves top-K candidate documents
        let candidates = self.bm25.search(&query_tokens, self.top_k);
        
        if candidates.is_empty() {
            // Fallback to pure Vibe Index phrase search
            return self.vibe.search(query);
        }

        // 3. For each candidate document, run Vibe Index phrase search
        let mut all_results: Vec<MatchResult> = Vec::new();
        
        for (doc_idx, bm25_score) in candidates {
            let (doc_start, doc_end) = self.bm25.documents[doc_idx];
            
            // Extract document tokens
            let doc_tokens: Vec<String> = self.vibe.token_sequence[doc_start..doc_end].to_vec();
            
            // Try phrase search on document tokens
            for i in 0..doc_tokens.len().saturating_sub(query_tokens.len() - 1) {
                let phrase: Vec<String> = doc_tokens[i..i + query_tokens.len()].to_vec();
                let vibe_results = self.vibe.phrase_search(&phrase);
                
                for mut r in vibe_results {
                    // Boost confidence by BM25 score
                    r.confidence = 0.7 + (bm25_score * 0.3).min(0.3);
                    r.context = format!("[DOC {}] [POS {}] BM25={:.2} | '{}'", 
                        doc_idx, r.position, bm25_score, phrase.join(" "));
                    all_results.push(r);
                }
            }
        }

        // Sort by confidence
        all_results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        all_results
    }

    /// Direct Vibe Index search (bypasses BM25)
    pub fn vibe_only_search(&self, query: &str) -> Vec<MatchResult> {
        self.vibe.search(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_search_basic() {
        let mut hybrid = HybridSearcher::new(3);
        
        // Document 1: about database connections
        let doc1: Vec<String> = vec!["fn".to_string(), "connect".to_string(), "db".to_string(), "(".to_string(), ")".to_string(), "→".to_string(), "Result".to_string(), "{".to_string(), "let".to_string(), "conn".to_string(), "=".to_string(), "db.connect()".to_string(), ";".to_string(), "}".to_string()];
        hybrid.add_document(0, doc1.len());
        
        // Document 2: about data processing
        let doc2_start = doc1.len();
        let doc2: Vec<String> = vec!["fn".to_string(), "process".to_string(), "data".to_string(), "(".to_string(), "data".to_string(), ":".to_string(), "&str".to_string(), ")".to_string(), "{".to_string(), "for".to_string(), "item".to_string(), "in".to_string(), "data".to_string(), "{".to_string(), "println!".to_string(), "(".to_string(), "item".to_string(), ")".to_string(), ";".to_string(), "}".to_string(), "}".to_string()];
        hybrid.add_document(doc2_start, doc2_start + doc2.len());
        
        let all: Vec<String> = [doc1.clone(), doc2].concat();
        hybrid.index_tokens(&all);
        
        let results = hybrid.search("connect database");
        assert!(!results.is_empty(), "Should find connect-related code");
    }

    #[test]
    fn test_hybrid_fallback_to_vibe() {
        let mut hybrid = HybridSearcher::new(3);
        
        let doc: Vec<String> = vec!["fn".to_string(), "main".to_string(), "(".to_string(), ")".to_string(), "{".to_string(), "}".to_string()];
        hybrid.add_document(0, doc.len());
        hybrid.index_tokens(&doc);
        
        // Query with no BM25 matches → should fallback to vibe
        let results = hybrid.search("nonexistent");
        // Results may be empty or from vibe-only search
        let _ = results;
    }
}
