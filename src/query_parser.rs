/// Query parser module
/// Transforms natural language queries into discrete search phrases for VibeIndex
use std::collections::HashSet;

/// English stop words to filter out from queries
pub const ENGLISH_STOP_WORDS: &[&str] = &[
    "a",
    "about",
    "above",
    "after",
    "again",
    "against",
    "all",
    "am",
    "an",
    "and",
    "any",
    "are",
    "as",
    "at",
    "be",
    "because",
    "been",
    "before",
    "being",
    "below",
    "between",
    "both",
    "but",
    "by",
    "can",
    "could",
    "did",
    "do",
    "does",
    "doing",
    "down",
    "during",
    "each",
    "few",
    "for",
    "from",
    "further",
    "get",
    "got",
    "had",
    "has",
    "have",
    "having",
    "he",
    "her",
    "here",
    "hers",
    "herself",
    "him",
    "himself",
    "his",
    "how",
    "i",
    "if",
    "in",
    "into",
    "is",
    "it",
    "its",
    "itself",
    "let's",
    "me",
    "might",
    "more",
    "most",
    "my",
    "myself",
    "nor",
    "not",
    "of",
    "off",
    "on",
    "once",
    "only",
    "or",
    "other",
    "ought",
    "our",
    "ours",
    "ourselves",
    "out",
    "over",
    "own",
    "s",
    "same",
    "she",
    "should",
    "so",
    "some",
    "such",
    "t",
    "than",
    "that",
    "the",
    "their",
    "theirs",
    "them",
    "themselves",
    "then",
    "there",
    "these",
    "they",
    "this",
    "those",
    "through",
    "to",
    "too",
    "under",
    "until",
    "up",
    "very",
    "was",
    "we",
    "were",
    "what",
    "when",
    "where",
    "which",
    "while",
    "who",
    "whom",
    "why",
    "will",
    "with",
    "would",
    "you",
    "your",
    "yours",
    "yourself",
    "yourselves",
];

/// Split a compound identifier into its component tokens
/// Handles: camelCase, PascalCase, snake_case, kebab-case, :: paths, generics
pub fn split_identifier(ident: &str) -> Vec<String> {
    let mut tokens = Vec::new();

    // Handle Rust/Go-style paths: std::collections::HashMap
    if ident.contains("::") {
        for part in ident.split("::") {
            tokens.extend(split_camel_case(part));
        }
        return tokens;
    }

    // Handle generic types and punctuation: Vec<String>, Result<T, E>
    let cleaned: String = ident.chars().filter(|c| !matches!(c, '<' | '>')).collect();
    if cleaned != ident {
        for part in cleaned.split(|c: char| c == ',' || c.is_whitespace()) {
            tokens.extend(split_camel_case(part));
        }
        return tokens;
    }

    tokens.extend(split_camel_case(ident));
    tokens
}

/// Split camelCase/PascalCase into individual words
/// Handles: fetchData -> ["fetch", "data"], HTTPSConnection -> ["https", "connection"], URL -> ["url"]
fn split_camel_case(ident: &str) -> Vec<String> {
    if ident.is_empty() {
        return Vec::new();
    }

    let mut tokens = Vec::new();

    // First split on _ and - as word boundaries
    for segment in ident.split(['_', '-']) {
        if segment.is_empty() {
            continue;
        }
        tokens.extend(split_camel_case_inner(segment));
    }

    tokens
}

/// Inner camelCase/PascalCase splitter (no _ or - handling)
fn split_camel_case_inner(ident: &str) -> Vec<String> {
    if ident.is_empty() {
        return Vec::new();
    }

    let mut tokens = Vec::new();
    let chars: Vec<char> = ident.chars().collect();

    // Find all split boundaries
    // Find camelCase boundaries
    let mut boundaries: Vec<usize> = Vec::new();
    for i in 1..chars.len() {
        let prev = chars[i - 1];
        let curr = chars[i];

        // lowercase -> uppercase OR uppercase->uppercase followed by lowercase
        if (prev.is_lowercase() && curr.is_uppercase())
            || (prev.is_uppercase()
                && curr.is_uppercase()
                && i + 1 < chars.len()
                && chars[i + 1].is_lowercase())
        {
            boundaries.push(i);
        }
    }

    // Split at boundaries
    let mut start = 0;
    for &bound in &boundaries {
        if start < bound {
            let segment = &ident[start..bound];
            if !segment.is_empty() {
                tokens.push(segment.to_lowercase());
            }
        }
        start = bound;
    }
    if start < ident.len() {
        let segment = &ident[start..];
        if !segment.is_empty() {
            tokens.push(segment.to_lowercase());
        }
    }

    tokens
}

/// Parse a natural language query into search phrases
/// Returns Vec<Vec<String> where each inner Vec is a phrase to search for
pub fn parse_query(query: &str) -> Vec<Vec<String>> {
    let stop_set: HashSet<&str> = ENGLISH_STOP_WORDS.iter().copied().collect();

    // Split on whitespace and punctuation, keeping alphanumeric tokens
    let raw_tokens: Vec<&str> = query
        .split(|c: char| !c.is_alphanumeric() && c != '.' && c != '\'' && c != '_')
        .filter(|s| !s.is_empty())
        .collect();

    // Process each token: split compound identifiers and remove stop words
    let mut all_tokens: Vec<String> = Vec::new();

    for token in &raw_tokens {
        let lower = token.to_lowercase();

        // Skip pure stop words
        if stop_set.contains(lower.as_str()) {
            continue;
        }

        // Skip single character tokens
        if token.len() == 1 && !token.contains('_') && !token.contains('-') {
            continue;
        }

        // Split compound identifiers into individual tokens
        let split = split_identifier(token);
        for t in split {
            if !stop_set.contains(t.as_str()) {
                all_tokens.push(t);
            }
        }
    }

    if all_tokens.is_empty() {
        return Vec::new();
    }

    // Group consecutive tokens into phrases (max 4 tokens per phrase)
    let mut phrases: Vec<Vec<String>> = Vec::new();
    let mut current_phrase: Vec<String> = Vec::new();

    for token in &all_tokens {
        current_phrase.push(token.clone());

        if current_phrase.len() >= 4 {
            phrases.push(std::mem::take(&mut current_phrase));
        }
    }

    if !current_phrase.is_empty() {
        phrases.push(current_phrase);
    }

    // Also add individual tokens as fallback phrases
    for token in &all_tokens {
        phrases.push(vec![token.clone()]);
    }

    phrases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_camel_case() {
        assert_eq!(split_camel_case("fetchData"), vec!["fetch", "data"]);
        assert_eq!(
            split_camel_case("HTTPSConnection"),
            vec!["https", "connection"]
        );
        assert_eq!(split_camel_case("getURL"), vec!["get", "url"]);
        assert_eq!(split_camel_case("Simple"), vec!["simple"]);
        assert_eq!(split_camel_case("XMLParser"), vec!["xml", "parser"]);
        // HashMap has uppercase M, so it splits at h→M
        assert_eq!(split_camel_case("HashMap"), vec!["hash", "map"]);
    }

    #[test]
    fn test_split_snake_case() {
        assert_eq!(split_identifier("fetch_data"), vec!["fetch", "data"]);
        assert_eq!(
            split_identifier("my_variable_name"),
            vec!["my", "variable", "name"]
        );
    }

    #[test]
    fn test_split_kebab_case() {
        assert_eq!(
            split_identifier("auth-middleware"),
            vec!["auth", "middleware"]
        );
    }

    #[test]
    fn test_split_path() {
        assert_eq!(
            split_identifier("std::collections::HashMap"),
            vec!["std", "collections", "hash", "map"]
        );
        assert_eq!(
            split_identifier("tokio::sync::Mutex"),
            vec!["tokio", "sync", "mutex"]
        );
    }

    #[test]
    fn test_split_generics() {
        assert_eq!(split_identifier("Vec<String>"), vec!["vec", "string"]);
        assert_eq!(split_identifier("Result<T, E>"), vec!["result", "t", "e"]);
    }

    #[test]
    fn test_parse_query_basic() {
        let phrases = parse_query("how does the auth middleware chain work");
        assert!(!phrases.is_empty(), "Should produce at least one phrase");
        let all_words: Vec<&str> = phrases
            .iter()
            .flat_map(|p| p.iter().map(|s| s.as_str()))
            .collect();
        assert!(all_words.contains(&"auth"));
        assert!(all_words.contains(&"middleware"));
        assert!(all_words.contains(&"chain"));
    }

    #[test]
    fn test_parse_query_removes_stop_words() {
        let phrases = parse_query("the quick brown fox");
        let all_words: Vec<&str> = phrases
            .iter()
            .flat_map(|p| p.iter().map(|s| s.as_str()))
            .collect();
        assert!(!all_words.contains(&"the"));
        assert!(all_words.contains(&"quick"));
        assert!(all_words.contains(&"brown"));
        assert!(all_words.contains(&"fox"));
    }

    #[test]
    fn test_parse_query_with_code() {
        let phrases = parse_query("where is the fetch_data function defined");
        let all_words: Vec<&str> = phrases
            .iter()
            .flat_map(|p| p.iter().map(|s| s.as_str()))
            .collect();
        assert!(all_words.contains(&"fetch"));
        assert!(all_words.contains(&"data"));
        assert!(all_words.contains(&"function"));
        assert!(all_words.contains(&"defined"));
    }

    #[test]
    fn test_parse_query_with_imports() {
        let phrases = parse_query("std::collections::HashMap usage");
        let all_words: Vec<&str> = phrases
            .iter()
            .flat_map(|p| p.iter().map(|s| s.as_str()))
            .collect();
        assert!(all_words.contains(&"std"));
        assert!(all_words.contains(&"collections"));
        assert!(all_words.contains(&"hash"));
        assert!(all_words.contains(&"map"));
    }

    #[test]
    fn test_parse_query_empty() {
        let phrases = parse_query("the a is");
        assert!(phrases.is_empty() || phrases.iter().all(|p| p.is_empty()));
    }

    #[test]
    fn test_parse_query_camelcase() {
        let phrases = parse_query("where is the fetchData function");
        let all_words: Vec<&str> = phrases
            .iter()
            .flat_map(|p| p.iter().map(|s| s.as_str()))
            .collect();
        assert!(all_words.contains(&"fetch"));
        assert!(all_words.contains(&"data"));
    }

    #[test]
    fn test_parse_query_mixed() {
        let phrases = parse_query("how does the auth middleware chain work in the pipeline");
        let all_words: Vec<&str> = phrases
            .iter()
            .flat_map(|p| p.iter().map(|s| s.as_str()))
            .collect();
        assert!(all_words.contains(&"auth"));
        assert!(all_words.contains(&"middleware"));
        assert!(all_words.contains(&"chain"));
        assert!(all_words.contains(&"pipeline"));
    }

    #[test]
    fn test_parse_query_returns_phrases_and_individual() {
        let phrases = parse_query("find the main function");
        assert!(phrases.len() >= 3, "Should return multiple search options");
    }
}
