use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Bm25Stats {
    pub doc_len: usize,
    pub avg_doc_len: f64,
    pub idf: HashMap<String, f64>,
}

/// Lightweight BM25 scoring for candidate retrieval.
/// Returns documents (position ranges) scored by relevance.
pub struct Bm25Index {
    /// Token frequency per document (position range)
    pub doc_freq: HashMap<String, Vec<(usize, usize)>>,
    /// All unique tokens
    all_tokens: Vec<String>,
    /// Document boundaries: (start_pos, end_pos)
    pub documents: Vec<(usize, usize)>,
    num_docs: usize,
    avg_doc_len: f64,
    k1: f64,  // term frequency saturation (default 1.5)
    b: f64,   // length normalization (default 0.75)
}

impl Bm25Index {
    pub fn new() -> Self {
        Self {
            doc_freq: HashMap::new(),
            all_tokens: Vec::new(),
            documents: Vec::new(),
            num_docs: 0,
            avg_doc_len: 0.0,
            k1: 1.5,
            b: 0.75,
        }
    }

    /// Add a document (token range) to the index
    pub fn add_document(&mut self, start: usize, end: usize) {
        self.documents.push((start, end));
        self.num_docs += 1;
    }

    /// Index tokens - call after all documents added
    pub fn index_tokens(&mut self, tokens: &[String]) {
        self.avg_doc_len = self.documents.iter()
            .map(|(s, e)| (e - s) as f64)
            .sum::<f64>() / self.num_docs as f64;

        // Build token frequency per document
        for (doc_idx, (start, end)) in self.documents.iter().enumerate() {
            for pos in *start..*end {
                let token = &tokens[pos];
                self.doc_freq
                    .entry(token.clone())
                    .or_insert_with(Vec::new)
                    .push((doc_idx, pos));
            }
        }

        // Calculate IDF for each token
        let idf_map: HashMap<String, f64> = self.doc_freq.iter()
            .map(|(token, occurrences)| {
                let df = occurrences.len();
                let idf = (((self.num_docs as f64) - (df as f64) + 0.5) / ((df as f64) + 0.5)).ln();
                (token.clone(), idf.max(0.0))
            })
            .collect();

        // Store in all_tokens for lookup
        self.all_tokens = idf_map.keys().cloned().collect();
    }

    /// Score a query against all documents. Returns top-K document indices by score.
    pub fn search(&self, query_tokens: &[String], top_k: usize) -> Vec<(usize, f64)> {
        if self.num_docs == 0 {
            return Vec::new();
        }

        let mut doc_scores: HashMap<usize, f64> = HashMap::new();

        for query_token in query_tokens {
            if let Some(occurrences) = self.doc_freq.get(query_token.as_str()) {
                        let idf = occurrences.iter()
                            .map(|(_, _pos)| {
                                // Simplified: calculate IDF on the fly
                                let df = occurrences.len();
                                (((self.num_docs as f64) - (df as f64) + 0.5) / ((df as f64) + 0.5)).ln().max(0.0)
                            })
                            .next()
                            .unwrap_or(0.0);

                for (doc_idx, _) in occurrences {
                    let doc_len = (self.documents[*doc_idx].1 - self.documents[*doc_idx].0) as f64;
                    let tf = occurrences.iter()
                        .filter(|(di, _)| *di == *doc_idx)
                        .count() as f64;

                    let score = idf * (tf * (self.k1 + 1.0))
                        / (tf + self.k1 * (1.0 - self.b + self.b * doc_len / self.avg_doc_len));

                    *doc_scores.entry(*doc_idx).or_insert(0.0) += score;
                }
            }
        }

        let mut scored: Vec<(usize, f64)> = doc_scores.into_iter().collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        scored.into_iter().take(top_k).collect()
    }
}
