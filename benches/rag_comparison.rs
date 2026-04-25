use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Instant;
use vibe_index::VibeIndex;

const RAG_RETRIEVAL_MS: f64 = 23.0;
const RAG_CHUNK_SIZE: usize = 1024;
const VIBE_WINDOW: usize = 10;

#[derive(Clone)]
struct Chunk {
    tokens: Vec<String>,
    _start: usize,
}

fn make_corpus(n: usize) -> Vec<String> {
    let mut out = Vec::with_capacity(n);
    let f0: Vec<&str> = vec!["fn", "authenticate", "(", "user", ":", "&str", ")", "->", "Result", "<", "Token", ">", "{",
         "let", "hash", "=", "bcrypt::hash", "(", "pass", ")", ";",
         "let", "row", "=", "db.query", "(", "\"SELECT * FROM users\"", ")", ";",
         "if", "bcrypt::verify", "(", "pass", ",", "&row.password", ")", "{",
         "Ok", "(", "jwt::encode", "(", "&row.id", ",", "&SECRET", ")", ")", "}",
         "else", "{", "Err", "(", "AuthError::InvalidCredentials", ")", "}", "}"];
    let f1: Vec<&str> = vec!["fn", "fetch_users", "(", "db", ":", "&Pool", ")", "->", "Result", "<", "Vec", "<", "User", ">", ">", "{",
         "let", "conn", "=", "db.get", "(", ")", ";",
         "let", "stmt", "=", "conn.prepare", "(", "\"SELECT id,username,email FROM users\"", ")", ";",
         "let", "mut", "rows", "=", "stmt.query", "(", ")", ";",
         "let", "mut", "users", ":", "Vec", "<", "User", ">", "=", "Vec::new", "(", ")", ";",
         "while", "let", "Some", "(", "user", ")", "=", "rows.next", "(", ")", "{", "users.push", "(", "user", ")", ";", "}",
         "Ok", "(", "users", ")", "}"];
    let f2: Vec<&str> = vec!["fn", "create_session", "(", "user_id", ":", "u64", ",", "ttl", ":", "Duration", ")", "->", "Session", "{",
         "let", "token", "=", "uuid::new_v4", "(", ")", ";",
         "redis.setex", "(", "\"session:\"", ",", "user_id.to_string", "(", ")", ",", "ttl.as_secs", "(", ")", ")", ";",
         "Session", "{", "token", ",", "user_id", ",", "expires_at", ":", "Utc::now", "(", ")", "+", "ttl", "}", "}"];
    let f3: Vec<&str> = vec!["fn", "middleware_chain", "(", "req", ":", "&mut", "Request", ")", "->", "Response", "{",
         "let", "auth", "=", "authenticate_request", "(", "req", ")", ";",
         "if", "auth.is_err", "(", ")", "{", "return", "Response::unauthorized", "(", ")", ";", "}",
         "let", "user", "=", "auth.unwrap", "(", ")", ";",
         "if", "!check_permissions", "(", "&user", ",", "req.path", ")", "{", "return", "Response::forbidden", "(", ")", ";", "}",
         "let", "rate", "=", "rate_limiter.check", "(", "&user.id", ")", ";",
         "if", "rate.exceeded", "{", "return", "Response::too_many_requests", "(", ")", ";", "}",
         "let", "start", "=", "Instant::now", "(", ")", ";",
         "let", "result", "=", "handle_request", "(", "req", ")", ";",
         "metrics.log_duration", "(", "req.path", ",", "start.elapsed", "(", ")", ")", ";",
         "result", "}"];
    let funcs: Vec<&[&str]> = vec![&f0, &f1, &f2, &f3];
    let fillers: &[&str] = &["let", "mut", "if", "else", "for", "while", "return", "match", "use", "pub",
                   "struct", "impl", "trait", "self", "String", "Vec", "Option", "Result",
                   "Some", "None", "Ok", "Err", "true", "false", "(", ")", "{", "}", "[", "]",
                   ";", ":", ",", ".", "<", ">", "=", "+", "-", "*", "/"];

    for i in 0..n {
        if i % 200 == 0 {
            let f = funcs[i / 200 % funcs.len()];
            for t in f { out.push(t.to_string()); }
        } else {
            out.push(fillers[i % fillers.len()].to_string());
        }
    }
    out
}

fn chunkify(tokens: &[String], size: usize) -> Vec<Chunk> {
    tokens.chunks(size).enumerate().map(|(i, b)| Chunk {
        tokens: b.to_vec(),
        _start: i * size,
    }).collect()
}

fn retrieve(chunks: &[Chunk], query: &[&str], top_k: usize) -> Vec<(usize, f64)> {
    let mut scored = Vec::new();
    for (idx, ch) in chunks.iter().enumerate() {
        let text = ch.tokens.join(" ").to_lowercase();
        let hits = query.iter().filter(|t| text.contains(**t)).count() as f64;
        if hits > 0.0 {
            scored.push((idx, hits / (ch.tokens.len() as f64).log10()));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    scored.into_iter().take(top_k).collect()
}

struct Res {
    tokens_injected: usize,
    latency: std::time::Duration,
}

fn rag_baseline(chunks: &[Chunk], query: &[&str]) -> Res {
    let t0 = Instant::now();
    std::thread::sleep(std::time::Duration::from_secs_f64(RAG_RETRIEVAL_MS / 1000.0));
    let hits = retrieve(chunks, query, 3);
    let injected: usize = hits.iter().map(|(i, _)| chunks[*i].tokens.len()).sum();
    Res { tokens_injected: injected, latency: t0.elapsed() }
}

fn rag_hybrid(chunks: &[Chunk], _all_tokens: &[String], query: &[&str]) -> Res {
    let t0 = Instant::now();
    std::thread::sleep(std::time::Duration::from_secs_f64(RAG_RETRIEVAL_MS / 1000.0));
    let hits = retrieve(chunks, query, 3);

    // Build Vibe Index per-chunk for accurate position tracking
    let mut total_injected: usize = 0;
    let q: Vec<String> = query.iter().map(|s| s.to_string()).collect();

    for (idx, _) in &hits {
        let chunk = &chunks[*idx];
        let mut vibe = VibeIndex::new();
        for t in &chunk.tokens { vibe.add_token(t); }

        let matches = vibe.phrase_search(&q);
        if !matches.is_empty() {
            // Inject only context windows around matches
            for m in &matches {
                let ws = m.position.saturating_sub(VIBE_WINDOW);
                let we = (m.position + q.len() + VIBE_WINDOW).min(chunk.tokens.len());
                total_injected += we.saturating_sub(ws);
            }
        } else {
            // Fallback: inject entire chunk
            total_injected += chunk.tokens.len();
        }
    }

    Res { tokens_injected: total_injected, latency: t0.elapsed() }
}

fn bench_group(c: &mut Criterion) {
    let tokens = make_corpus(50_000);
    let chunks = chunkify(&tokens, RAG_CHUNK_SIZE);

    let mut g = c.benchmark_group("rag_comparison");
    g.sample_size(40);

    g.bench_function("fn_authenticate_rag", |b| b.iter(|| black_box(rag_baseline(&chunks, &["fn", "authenticate"]))));
    g.bench_function("fn_authenticate_hybrid", |b| b.iter(|| black_box(rag_hybrid(&chunks, &tokens, &["fn", "authenticate"]))));
    g.bench_function("let_hash_rag", |b| b.iter(|| black_box(rag_baseline(&chunks, &["let", "hash"]))));
    g.bench_function("let_hash_hybrid", |b| b.iter(|| black_box(rag_hybrid(&chunks, &tokens, &["let", "hash"]))));
    g.bench_function("let_conn_rag", |b| b.iter(|| black_box(rag_baseline(&chunks, &["let", "conn"]))));
    g.bench_function("let_conn_hybrid", |b| b.iter(|| black_box(rag_hybrid(&chunks, &tokens, &["let", "conn"]))));
    g.bench_function("ok_paren_rag", |b| b.iter(|| black_box(rag_baseline(&chunks, &["Ok", "("]))));
    g.bench_function("ok_paren_hybrid", |b| b.iter(|| black_box(rag_hybrid(&chunks, &tokens, &["Ok", "("]))));
    g.bench_function("no_match_fallback_rag", |b| b.iter(|| black_box(rag_baseline(&chunks, &["cursor", "execute"]))));
    g.bench_function("no_match_fallback_hybrid", |b| b.iter(|| black_box(rag_hybrid(&chunks, &tokens, &["cursor", "execute"]))));
    g.finish();
}

#[test]
fn rag_summary() {
    let tokens = make_corpus(50_000);
    let chunks = chunkify(&tokens, RAG_CHUNK_SIZE);
    // Queries use actual consecutive token pairs from corpus patterns
    let queries: Vec<(Vec<&str>, &str)> = vec![
        (vec!["fn", "authenticate"], "fn_authenticate"),
        (vec!["let", "hash"], "let_hash"),
        (vec!["let", "conn"], "let_conn"),
        (vec!["Ok", "("], "ok_paren"),
        (vec!["cursor", "execute"], "no_match_fallback"),
    ];

    println!("\n{:-<90}", "");
    println!("{:<25} {:>12} {:>12} {:>12}", "Query", "RAG tok", "Hybrid tok", "Reduction");
    println!("{:-<90}", "");

    let mut tr: usize = 0;
    let mut th: usize = 0;
    for (q, label) in &queries {
        let r = rag_baseline(&chunks, q);
        let h = rag_hybrid(&chunks, &tokens, q);
        let red = if r.tokens_injected > 0 { (1.0 - h.tokens_injected as f64 / r.tokens_injected as f64) * 100.0 } else { 0.0 };
        println!("{:<25} {:>12} {:>12} {:>11.1}%", label, r.tokens_injected, h.tokens_injected, red);
        tr += r.tokens_injected;
        th += h.tokens_injected;
    }
    let tred = if tr > 0 { (1.0 - th as f64 / tr as f64) * 100.0 } else { 0.0 };
    println!("{:-<90}", "");
    println!("{:<25} {:>12} {:>12} {:>11.1}%", "TOTAL", tr, th, tred);
    println!("{:-<90}\n", "");
}

criterion_group!(benches, bench_group);
criterion_main!(benches, rag_summary);
