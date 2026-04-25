# Vibe Index

**Sub-millisecond exact phrase retrieval for AI-assisted development.**

[![CI](https://github.com/mladenpop-oss/vibe-index/actions/workflows/ci.yml/badge.svg)](https://github.com/mladenpop-oss/vibe-index/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Stars](https://img.shields.io/github/stars/mladenpop-oss/vibe-index?style=social)](https://github.com/mladenpop-oss/vibe-index)

> Instead of embeddings and vector search, each token maps to a compressed bitmap of its positions. Phrase matching becomes a bitwise operation over these bitmaps.

## Why?

Traditional RAG adds 8-12K tokens to each LLM prompt, most of which are irrelevant. With a 20B model eating all your VRAM, each unnecessary context injection pushes you over the edge.

Vibe Index replaces "similar" code retrieval with **exact** code at **exact** positions — in microseconds.

## Results

| Metric | RAG (baseline) | Vibe Index |
|--------|----------------|------------|
| Context tokens per query | 8K-16K | 3K-6K |
| Search latency | 50-300 ms | 4-30 µs |
| Exact phrase match | No | Yes |
| Typo tolerance | No | Yes |
| VRAM pressure | High (linear KV cache growth) | Low (dense, optimized) |

Benchmarked on 50K token corpus, single core, release build.

## How it works

1. Each unique token maps to a **Roaring Bitmap** of its positions
2. Phrase matching uses **anchor-bitmap + contains()** over these bitmaps
3. Results include surrounding context automatically
4. Bitmap compression means it gets **faster** as your codebase grows

## Hybrid Search

Use BM25 for candidate retrieval → Vibe Index for exact position validation:

```rust
use vibe_index::hybrid_search::HybridSearcher;

let mut hybrid = HybridSearcher::new(5); // top-5 candidates
for doc in &documents {
    hybrid.add_document(start, end);
}
hybrid.index_tokens(&all_tokens);

// BM25 finds relevant docs → Vibe validates exact phrases
let results = hybrid.search("connect database");
```

## How it compares

Vibe Index is best viewed as a **precision-oriented lexical retrieval primitive**, not a replacement for all retrieval methods.

| Approach | What it matches | Best for |
|----------|----------------|----------|
| **Vibe Index** | Exact token positions + simple typo matches | Exact phrase/code lookup, sub-millisecond |
| **BM25** | Lexical term overlap | Keyword search, ranked document retrieval |
| **Embeddings** | Semantic similarity between chunks | Conceptual search with different words |
| **ColBERT** | Token-level neural similarity | Semantic + lexical matching |

**Vibe Index vs BM25**

BM25 answers: "Which chunks are probably relevant to 'database cursor execute'?"
Vibe Index answers: "Where exactly does ['cursor', 'execute'] occur?"

**Vibe Index vs Embeddings**

Embeddings find related code even if the words differ. Vibe Index finds exact code at exact positions. For code search, Vibe Index is sharper when you know the phrase or symbol.

**Vibe Index vs ColBERT**

ColBERT: neural token embeddings + max-sim scoring
Vibe Index: exact token bitmap operations

ColBERT matches related meanings and paraphrases. Vibe Index is simpler, cheaper, and faster.

**The hybrid approach**

```
BM25 or embeddings: find likely files/chunks
Vibe Index: pinpoint exact symbols/phrases/positions inside them
```

## Query Parser

Natural language queries are automatically converted to search phrases:

```rust
use vibe_index::query_parser::parse_query;

let phrases = parse_query("how does the auth middleware chain work?");
// → [["auth", "middleware", "chain"], ["auth"], ["middleware"], ["chain"]]
```

Handles: camelCase, PascalCase, snake_case, kebab-case, `::` paths (`std::collections::HashMap`), generics (`Vec<String>`), acronyms (HTTPS, URL).

## Unified Search

For most use cases, use `VibeIndex::search()` — it combines everything:

```rust
// One line: natural language → ranked results
let results = index.search("where is the auth middleware chain");
// → [MatchResult { position: 42, confidence: 0.95 }, ...]
```

Behind the scenes it:
1. Parses the query into search phrases
2. Runs exact phrase matching
3. Falls back to fuzzy matching for typos
4. Merges and sorts by confidence

## Llama.cpp Integration

Full pipeline: index code → extract phrases → build prompt → get LLM completion.

```rust
use vibe_index::llama_cpp::LlamaCppIntegration;

let mut integration = LlamaCppIntegration::new("http://127.0.0.1:8080".into());

// Index your codebase
for token in &your_tokens {
    integration.add_token(token);
}

// Full pipeline: index + search + LLM completion
let (response, matches) = integration.ask(
    &context_tokens,
    "What does the add function do?",
    &search_queries,
).await?;
```

Tested with Qwen3VL-4B. Search completes in ~60µs, LLM generates the answer.

## vLLM Integration

Production-ready integration with hybrid search, context budget management, and output validation.

```rust
use vibe_index::vllm::{VllmIntegration, prompts};

let mut integration = VllmIntegration::new(
    "http://127.0.0.1:8000".into(), // vLLM server URL
    4096,                            // max context tokens
);

// Index your codebase
for token in &your_tokens {
    integration.add_token(token);
}
for (start, end) in &document_ranges {
    integration.add_document(*start, *end);
}
integration.index_tokens(&all_tokens);

// Full pipeline: hybrid search → build messages → vLLM → validate output
let (response, matches, ctx_validation, output_validation) = integration.ask(
    &context_tokens,
    "Refactor the auth middleware to use JWT",
    &[vec!["auth".into(), "middleware".into()]],
).await?;

// Check for issues
if !output_validation.syntax_valid {
    println!("Output issues: {:?}", output_validation.issues);
}
```

**Features:**
- **Hybrid search** — BM25 candidate retrieval + Vibe Index exact position validation
- **Context window budget** — automatic truncation when context exceeds token limit
- **Post-injection validation** — checks for truncated tokens, unbalanced braces/parens
- **Output sanity checks** — validates syntax of generated code (braces, parens, terminators)
- **Confidence feedback loop** — tracks success rate per query, adjusts future weights

## Live Demo

See it in action: [Interactive Demo](https://mladenpop-oss.github.io/vibe-index/demo.html)

## Quick start

```bash
# Clone and build
git clone https://github.com/mladenpop-oss/vibe-index.git
cd vibe-index
cargo run --release

# Run benchmarks
cargo bench

# Or use the benchmark runner script
./run_benchmark.sh  # Linux/Mac
powershell -ExecutionPolicy Bypass -File .\run_benchmark.ps1  # Windows
```

## Benchmarks

| Benchmark | Time |
|-----------|------|
| Index 50K tokens | ~16 ms |
| Phrase match (1 occurrence) | ~310 ns |
| Phrase match (100 occurrences) | ~18 µs |
| Phrase not found | ~190 ns |
| Unified natural language search | ~2.6 ms |
| Hybrid search (BM25 + Vibe) | ~105-147 µs |

*All benchmarks run on release build, single core.*

## Contributing

We welcome contributions! Please:
1. Fork the repo
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Run tests and benchmarks (`cargo test && cargo bench`)
4. Commit your changes (`git commit -m 'feat: add amazing feature'`)
5. Push to the branch (`git push origin feature/amazing-feature`)
6. Open a Pull Request

## License

MIT
