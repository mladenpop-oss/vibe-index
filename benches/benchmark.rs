use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vibe_index::{hybrid_search::HybridSearcher, VibeIndex};

fn generate_large_codebase(num_tokens: usize) -> Vec<String> {
    let mut tokens = Vec::with_capacity(num_tokens);
    let common_tokens = vec![
        "fn", "let", "mut", "if", "else", "for", "while", "return", "match", "use", "pub",
        "struct", "impl", "trait", "enum", "async", "await", "self", "Self", "String", "Vec",
        "Option", "Result", "Some", "None", "Ok", "Err", "true", "false", "println!", "format!",
        "debug!", "info!", "warn!", "error!", "(", ")", "{", "}", "[", "]", ";", ":", ",", ".",
        "<", ">", "=", "+", "-", "*", "/",
    ];

    // Simulate realistic code patterns with some repetition
    for i in 0..num_tokens {
        if i % 100 == 0 {
            // Every 100 tokens, add a function signature pattern
            tokens.push("fn".to_string());
            tokens.push("process_".to_string());
            tokens.push(format!("item_{}", i / 100));
            tokens.push(":".to_string());
            tokens.push("&".to_string());
            tokens.push("str".to_string());
            tokens.push(")".to_string());
            tokens.push("{".to_string());
        } else if i % 50 == 0 {
            // Every 50 tokens, add a variable assignment
            tokens.push("let".to_string());
            tokens.push("mut".to_string());
            tokens.push("result".to_string());
            tokens.push("=".to_string());
            tokens.push("self".to_string());
            tokens.push(".".to_string());
            tokens.push("method".to_string());
            tokens.push("(".to_string());
            tokens.push(")".to_string());
            tokens.push(";".to_string());
        } else {
            // Random common token
            let idx = i % common_tokens.len();
            tokens.push(common_tokens[idx].to_string());
        }
    }

    tokens
}

fn bench_indexing(c: &mut Criterion) {
    c.bench_function("index_50k_tokens", |b| {
        let tokens = generate_large_codebase(50_000);
        b.iter(|| {
            let mut index = VibeIndex::new();
            for token in &tokens {
                index.add_token(token);
            }
            black_box(index)
        })
    });

    c.bench_function("index_10k_tokens", |b| {
        let tokens = generate_large_codebase(10_000);
        b.iter(|| {
            let mut index = VibeIndex::new();
            for token in &tokens {
                index.add_token(token);
            }
            black_box(index)
        })
    });
}

fn bench_phrase_search(c: &mut Criterion) {
    let mut index = VibeIndex::new();
    let tokens = generate_large_codebase(50_000);
    for token in &tokens {
        index.add_token(token);
    }

    c.bench_function("search_fn_process_item", |b| {
        b.iter(|| {
            let results = index.phrase_search(&["fn".into(), "process_0".into()]);
            black_box(results)
        })
    });

    c.bench_function("search_let_mut_result", |b| {
        b.iter(|| {
            let results = index.phrase_search(&["let".into(), "mut".into(), "result".into()]);
            black_box(results)
        })
    });

    c.bench_function("search_nonexistent_phrase", |b| {
        b.iter(|| {
            let results = index.phrase_search(&["nonexistent".into(), "phrase".into()]);
            black_box(results)
        })
    });
}

fn bench_fuzzy_search(c: &mut Criterion) {
    let mut index = VibeIndex::new();
    let tokens = generate_large_codebase(50_000);
    for token in &tokens {
        index.add_token(token);
    }

    c.bench_function("fuzzy_search_typo_1char", |b| {
        b.iter(|| {
            let results = index.fuzzy_search("proces", 1);
            black_box(results)
        })
    });

    c.bench_function("fuzzy_search_typo_2char", |b| {
        b.iter(|| {
            let results = index.fuzzy_search("proces", 2);
            black_box(results)
        })
    });

    c.bench_function("fuzzy_search_no_match", |b| {
        b.iter(|| {
            let results = index.fuzzy_search("xyzabc", 1);
            black_box(results)
        })
    });
}

fn bench_unified_search(c: &mut Criterion) {
    let mut index = VibeIndex::new();
    let tokens = generate_large_codebase(50_000);
    for token in &tokens {
        index.add_token(token);
    }

    c.bench_function("unified_search_natural_lang", |b| {
        b.iter(|| {
            let results = index.search("where is the process_item function");
            black_box(results)
        })
    });

    c.bench_function("unified_search_typo_tolerance", |b| {
        b.iter(|| {
            let results = index.search("where is the proces_item fuction");
            black_box(results)
        })
    });
}

fn bench_hybrid_search(c: &mut Criterion) {
    let mut hybrid = HybridSearcher::new(5);

    // Create 10 documents of 500 tokens each
    let mut all_tokens = Vec::new();
    for doc_idx in 0..10 {
        let start = all_tokens.len();
        for i in 0..500 {
            if i % 100 == 0 {
                all_tokens.push(format!("fn process_item_{}", doc_idx));
            } else {
                for word in &[
                    "let", "mut", "result", "=", "self", ".", "method", "(", ")", ";", "true",
                    "false",
                ] {
                    all_tokens.push(word.to_string());
                }
            }
        }
        hybrid.add_document(start, all_tokens.len());
    }
    hybrid.index_tokens(&all_tokens);

    c.bench_function("hybrid_search_connect_db", |b| {
        b.iter(|| {
            let results = hybrid.search("connect database");
            black_box(results)
        })
    });

    c.bench_function("hybrid_search_process_item", |b| {
        b.iter(|| {
            let results = hybrid.search("process item function");
            black_box(results)
        })
    });

    c.bench_function("vibe_only_fallback", |b| {
        b.iter(|| {
            let results = hybrid.vibe_only_search("nonexistent phrase");
            black_box(results)
        })
    });
}

criterion_group!(
    benches,
    bench_indexing,
    bench_phrase_search,
    bench_fuzzy_search,
    bench_unified_search,
    bench_hybrid_search
);
criterion_main!(benches);
