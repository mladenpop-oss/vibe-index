use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Instant;
use vibe_index::VibeIndex;

/// Simple Rust tokenizer: extracts identifiers, keywords, and meaningful tokens
/// from source code. Strips strings, comments, and numeric literals.
fn tokenize_rust_source(source: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();

    while let Some(c) = chars.next() {
        // Skip string literals
        if c == '"' {
            while let Some(nc) = chars.next() {
                if nc == '\\' {
                    chars.next(); // skip escaped char
                } else if nc == '"' {
                    break;
                }
            }
            tokens.push("STR".to_string());
            continue;
        }

        // Skip raw string literals
        if c == 'r' {
            let mut hash_count = 0usize;
            while chars.peek() == Some(&'#') {
                chars.next();
                hash_count += 1;
            }
            if let Some(&'"') = chars.peek() {
                chars.next();
                let closing = "#".repeat(hash_count) + "\"";
                let mut buf = String::new();
                for nc in chars.by_ref() {
                    buf.push(nc);
                    if buf.ends_with(&closing) {
                        break;
                    }
                }
                tokens.push("RAW_STR".to_string());
                continue;
            }
        }

        // Skip character literals
        if c == '\'' {
            while let Some(nc) = chars.next() {
                if nc == '\\' {
                    chars.next();
                } else if nc == '\'' {
                    break;
                }
            }
            tokens.push("CHAR".to_string());
            continue;
        }

        // Skip line comments
        if c == '/' && chars.peek() == Some(&'/') {
            for nc in chars.by_ref() {
                if nc == '\n' {
                    break;
                }
            }
            continue;
        }

        // Skip block comments
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(nc) = chars.next() {
                if nc == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
            continue;
        }

        // Skip whitespace
        if c.is_whitespace() {
            continue;
        }

        // Identifiers and keywords
        if c.is_alphabetic() || c == '_' {
            let mut ident = String::from(c);
            while let Some(&nc) = chars.peek() {
                if nc.is_alphanumeric() || nc == '_' {
                    ident.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            tokens.push(ident);
            continue;
        }

        // Multi-char operators
        if c == '/' && chars.peek() == Some(&'=') {
            chars.next();
            tokens.push("/=".to_string());
            continue;
        }
        if c == '*' && chars.peek() == Some(&'=') {
            chars.next();
            tokens.push("*=".to_string());
            continue;
        }
        if c == '+' && chars.peek() == Some(&'=') {
            chars.next();
            tokens.push("+=".to_string());
            continue;
        }
        if c == '-' && chars.peek() == Some(&'=') {
            chars.next();
            tokens.push("-=".to_string());
            continue;
        }
        if c == '-' && chars.peek() == Some(&'>') {
            chars.next();
            tokens.push("->".to_string());
            continue;
        }
        if c == '=' && chars.peek() == Some(&'=') {
            chars.next();
            tokens.push("==".to_string());
            continue;
        }
        if c == '!' && chars.peek() == Some(&'=') {
            chars.next();
            tokens.push("!=".to_string());
            continue;
        }
        if c == '<' && chars.peek() == Some(&'=') {
            chars.next();
            tokens.push("<=".to_string());
            continue;
        }
        if c == '>' && chars.peek() == Some(&'=') {
            chars.next();
            tokens.push(">=".to_string());
            continue;
        }
        if c == '<' && chars.peek() == Some(&'<') {
            chars.next();
            if chars.peek() == Some(&'=') {
                chars.next();
                tokens.push("<<=".to_string());
            } else {
                tokens.push("<<".to_string());
            }
            continue;
        }
        if c == '>' && chars.peek() == Some(&'>') {
            chars.next();
            if chars.peek() == Some(&'=') {
                chars.next();
                tokens.push(">>=".to_string());
            } else {
                tokens.push(">>".to_string());
            }
            continue;
        }
        if c == '&' && chars.peek() == Some(&'&') {
            chars.next();
            tokens.push("&&".to_string());
            continue;
        }
        if c == '|' && chars.peek() == Some(&'|') {
            chars.next();
            tokens.push("||".to_string());
            continue;
        }
        if c == '%' && chars.peek() == Some(&'=') {
            chars.next();
            tokens.push("%=".to_string());
            continue;
        }
        if c == '^' && chars.peek() == Some(&'=') {
            chars.next();
            tokens.push("^=".to_string());
            continue;
        }
        if c == '.' && chars.peek() == Some(&'.') {
            chars.next();
            if chars.peek() == Some(&'.') {
                chars.next();
                tokens.push("...".to_string());
            } else {
                tokens.push("..".to_string());
            }
            continue;
        }
        if c == ':' && chars.peek() == Some(&':') {
            chars.next();
            tokens.push("::".to_string());
            continue;
        }
        if c == '+' && chars.peek() == Some(&'+') {
            chars.next();
            tokens.push("++".to_string());
            continue;
        }
        if c == '-' && chars.peek() == Some(&'-') {
            chars.next();
            tokens.push("--".to_string());
            continue;
        }

        // Single-char tokens (operators, punctuation)
        tokens.push(c.to_string());
    }

    tokens
}

/// Walk a directory and collect all .rs file contents
fn collect_rust_files(dir: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("rs")
            && !path.starts_with("target/")
            && !path.starts_with(".git/")
        {
            if let Ok(content) = fs::read_to_string(path) {
                let rel = path
                    .strip_prefix(dir)
                    .ok()
                    .and_then(|p| p.to_str())
                    .unwrap_or(path.to_str().unwrap_or(""))
                    .to_string();
                files.push((rel, content));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

#[test]
fn test_real_codebase_indexing() {
    // Index the vibe-index codebase itself
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = crate_root.join("src");

    let files = collect_rust_files(&src_dir);
    eprintln!("Found {} .rs files", files.len());

    let mut index = VibeIndex::new();
    let t0 = Instant::now();

    for (path, content) in &files {
        let tokens = tokenize_rust_source(content);
        eprintln!("  {} → {} tokens", path, tokens.len());
        for token in tokens {
            index.add_token(&token);
        }
    }

    let indexing_time = t0.elapsed();
    eprintln!(
        "\nIndexing complete: {} total tokens, {} unique tokens",
        index.total_positions(),
        index.unique_tokens()
    );
    eprintln!("Indexing time: {:?}", indexing_time);
    eprintln!(
        "Estimated memory: {:.2} MB",
        index.estimated_memory_bytes() as f64 / 1_048_576.0
    );

    assert!(
        index.total_positions() > 1000,
        "Should have indexed many tokens"
    );
}

#[test]
fn test_real_phrase_search() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = crate_root.join("src");
    let files = collect_rust_files(&src_dir);

    let mut index = VibeIndex::new();
    for (_, content) in &files {
        let tokens = tokenize_rust_source(content);
        for token in tokens {
            index.add_token(&token);
        }
    }

    // Test 1: Find function definition
    let t0 = Instant::now();
    let results = index.phrase_search(&["fn".into(), "tokenize_rust_source".into()]);
    let search_time = t0.elapsed();
    eprintln!(
        "Search 'fn tokenize_rust_source': {} matches in {:?}",
        results.len(),
        search_time
    );
    assert!(
        results.is_empty(),
        "This test function is only in integration test"
    );

    // Test 2: Find common pattern that should exist
    let t0 = Instant::now();
    let results = index.phrase_search(&["pub".into(), "fn".into(), "new".into()]);
    let search_time = t0.elapsed();
    eprintln!(
        "Search 'pub fn new': {} matches in {:?}",
        results.len(),
        search_time
    );
    assert!(!results.is_empty(), "Should find 'pub fn new' patterns");

    // Test 3: Find impl Default for VibeIndex
    let t0 = Instant::now();
    let results = index.phrase_search(&["impl".into(), "Default".into(), "for".into()]);
    let search_time = t0.elapsed();
    eprintln!(
        "Search 'impl Default for': {} matches in {:?}",
        results.len(),
        search_time
    );
    assert!(
        !results.is_empty(),
        "Should find 'impl Default for' pattern"
    );

    // Test 4: Find common code pattern
    let results = index.phrase_search(&["let".into(), "mut".into(), "self".into()]);
    eprintln!("Search 'let mut self': {} matches", results.len());

    // Test 5: Unified search
    let t0 = Instant::now();
    let results = index.search("where is the phrase search function");
    let search_time = t0.elapsed();
    eprintln!(
        "Unified search 'phrase search function': {} results in {:?}",
        results.len(),
        search_time
    );
    assert!(!results.is_empty(), "Should find phrase-related code");

    // Test 6: Fuzzy search with typo
    let t0 = Instant::now();
    let results = index.search("whre is the pharse searsh");
    let search_time = t0.elapsed();
    eprintln!(
        "Fuzzy search 'pharse searsh': {} results in {:?}",
        results.len(),
        search_time
    );
}

#[test]
fn test_real_codebase_stats() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = crate_root.join("src");
    let files = collect_rust_files(&src_dir);

    let mut index = VibeIndex::new();
    let mut all_tokens: HashSet<String> = HashSet::new();

    for (_, content) in &files {
        let tokens = tokenize_rust_source(content);
        for token in &tokens {
            all_tokens.insert(token.clone());
            index.add_token(token);
        }
    }

    eprintln!("\n=== Codebase Statistics ===");
    eprintln!("Files: {}", files.len());
    eprintln!("Total tokens: {}", index.total_positions());
    eprintln!("Unique tokens: {}", index.unique_tokens());
    eprintln!("Lexicon size: {}", index.lexicon.len());
    eprintln!(
        "Memory: {:.2} MB",
        index.estimated_memory_bytes() as f64 / 1_048_576.0
    );

    // Top 20 most frequent tokens (by bitmap cardinality)
    let mut freq: Vec<(u32, &str, u64)> = index
        .token_positions
        .iter()
        .filter_map(|(&id, bitmap)| index.lexicon.get_token(id).map(|t| (id, t, bitmap.len())))
        .collect();
    freq.sort_by_key(|b| std::cmp::Reverse(b.2));

    eprintln!("\nTop 20 tokens by frequency:");
    for (i, (_, token, count)) in freq.iter().take(20).enumerate() {
        eprintln!("  {:>3}. {:<15} ×{}", i + 1, token, count);
    }
}
