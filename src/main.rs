use vibe_index::VibeIndex;
use vibe_index::query_parser::parse_query;
use std::time::Instant;

fn main() {
    println!("=== Vibe Index Demo ===\n");

    let codebase = r#"
        fn fetch_data(db_url: &str) -> Result<Vec<Row>, Error> {
            let conn = sqlite3::connect(db_url)?;
            let cursor = conn.cursor();
            cursor.execute("SELECT * FROM users")?;
            Ok(cursor.fetchall())
        }

        fn main() {
            let data = fetch_data("app.db").unwrap();
            println!("Found {} records", data.len());
        }
    "#;

    let tokens: Vec<String> = codebase
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    println!("[1] Indexing {} tokens...", tokens.len());
    let mut index = VibeIndex::new();
    for token in &tokens {
        index.add_token(token);
    }
    println!("    Indexed {} unique tokens\n", index.unique_tokens());

    // Query Parser
    println!("[2] Query Parser (natural language -> search phrases):");
    let nl_queries = vec![
        "where is the fetch_data function",
        "how does the cursor execute work",
        "find the main function",
    ];
    for query in &nl_queries {
        let phrases = parse_query(query);
        println!("    Query: \"{}\"", query);
        println!("    Phrases: {:?}", phrases);
        println!();
    }

    // Unified Search
    println!("[3] Unified Search (natural language -> ranked results):");
    let search_queries = vec![
        "where is the fetch_data function",
        "how does the cursor execute work",
        "find the main function",
        "where is the fetchdt function", // typo
    ];
    for query in &search_queries {
        let results = index.search(query);
        println!("    Query: \"{}\"", query);
        println!("    Results: {} matches", results.len());
        for r in results.iter().take(3) {
            println!("      [POS {}] conf={:.2} {}", r.position, r.confidence, r.context);
        }
        println!();
    }

    // Phrase search (low-level)
    println!("[4] Phrase search (low-level API):");
    let queries = vec![
        vec!["fn".into(), "fetch_data".into()],
        vec!["cursor".into(), "execute".into()],
        vec!["let".into(), "conn".into()],
    ];

    for query in &queries {
        let results = index.phrase_search(query);
        println!("    '{}' -> {} matches", query.join(" "), results.len());
        for r in results {
            println!("      [POS {}] {}", r.position, r.context);
        }
    }

    // Fuzzy search
    println!("\n[5] Fuzzy search (typo tolerance):");
    let typos = vec![("execut", 2), ("fetc", 2), ("recors", 2)];
    for (query, max_dist) in typos {
        let results = index.fuzzy_search(query, max_dist);
        println!("    '{}' (max_dist={}) -> {} matches", query, max_dist, results.len());
        for r in results {
            println!("      {} (conf={:.2})", r.context, r.confidence);
        }
    }

    // Benchmark
    println!("\n[6] Benchmark (100,000 iterations):");
    let bench_queries = vec![
        vec!["fn".into()],
        vec!["cursor".into()],
        vec!["fn".into(), "fetch_data".into()],
        vec!["cursor".into(), "execute".into()],
    ];

    for query in &bench_queries {
        let start = Instant::now();
        for _ in 0..100_000 {
            let _ = index.phrase_search(query);
        }
        let elapsed = start.elapsed();
        let avg_ns = elapsed.as_nanos() / 100_000u32 as u128;
        println!("    '{}' -> avg {}ns ({:.2}ms total)",
            query.join(" "), avg_ns, elapsed.as_secs_f64() * 1000.0);
    }

    println!("\n=== Done ===");
}
