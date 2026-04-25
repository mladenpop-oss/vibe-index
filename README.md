# Vibe Index

**Roaring Bitmap positional phrase matching for low-latency LLM context retrieval.**

[![CI](https://github.com/mladenpop-oss/vibe-index/actions/workflows/ci.yml/badge.svg)](https://github.com/mladenpop-oss/vibe-index/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org)

> Each token maps to a compressed Roaring Bitmap of its positions. Phrase matching becomes an anchor-and-offset scan over bitmaps — no embeddings, no vectors.

## Why

RAG pipelines stuff 8K-16K tokens into every LLM prompt, most irrelevant. Vibe Index finds **exact** phrases at **exact** positions in microseconds, then injects only the relevant context window.

## Benchmarks (verified)

Measured on 50K token synthetic codebase, release build, single core, Windows 11:

| Benchmark | Median time |
|-----------|-------------|
| Index 50K tokens | **5.75 ms** |
| Index 10K tokens | **1.13 ms** |
| Phrase match — 1 occurrence (`fn process_0`) | **117 ns** |
| Phrase match — ~100 occurrences (`let mut result`) | **170 µs** |
| Phrase not found (early exit) | **70 ns** |
| Unified NL search (`where is the process_item function`) | **977 µs** |
| Unified search + typo tolerance (`proces_item fuction`) | **851 µs** |
| Hybrid search — BM25 + Vibe (`connect database`) | **36 µs** |
| Hybrid search — multi-match (`process item function`) | **47 µs** |
| Vibe-only fallback (no BM25 hit) | **38 µs** |

**Tests: 41/41 passing** (39 unit + 2 llama.cpp integration)

## Architecture

```
Token stream → Roaring Bitmap per unique token → Phrase search via anchor bitmap + offset validation
```

1. `add_token("foo")` → bitmap for "foo" gets current position pushed
2. `phrase_search(["foo", "bar"])` → pick smallest bitmap as anchor, iterate its positions, check if sibling tokens exist at `pos + offset`
3. Roaring Bitmap internal run-length compression keeps memory sub-linear

## Quick start

```bash
git clone https://github.com/mladenpop-oss/vibe-index.git
cd vibe-index
cargo run --release
```

```rust
use vibe_index::VibeIndex;

let mut index = VibeIndex::new();
for token in &["fn", "main", "(", ")", "{", "println!", "(", "\"hello\"", ")"] {
    index.add_token(token);
}

// Exact phrase search
let results = index.phrase_search(&["println".into(), "(".into()]);

// Unified search: NL query → phrase + fuzzy → ranked results
let results = index.search("where is the println call");
```

## API surface

| Module | Purpose |
|--------|---------|
| `VibeIndex` | Core: `add_token`, `phrase_search`, `fuzzy_search`, `search` |
| `query_parser` | NL → phrases: splits camelCase, snake_case, `::` paths, generics, strips stop words |
| `bm25` | Lightweight BM25 scorer for document-level candidate ranking |
| `hybrid_search` | BM25 candidates → Vibe Index exact position validation |
| `hot_cold` | In-memory hot buffer + disk-backed cold segments with persisted bitmaps and cross-layer phrase search |
| `persistent_storage` | Gzip-compressed token sequences + serialized bitmaps, magic byte validation, v2 format with backward compat |
| `prompt_injector` | Context window builder: search → filter by confidence → extract windows → build prompt |
| `llama_cpp` | Full pipeline: index → search → build prompt → llama.cpp completion |
| `vllm` | Hybrid search + context budget + output validation + confidence feedback loop |

## How it compares

| Approach | Matches | Latency | Use case |
|----------|---------|---------|----------|
| **Vibe Index** | Exact token positions + typo tolerance | 70ns - 977µs | Known phrase/symbol lookup |
| **BM25** | Lexical term overlap (document-level) | ms | Keyword search, ranked retrieval |
| **Embeddings** | Semantic similarity | 10-100ms | Conceptual search, different wording |

**Hybrid pattern:** BM25/embeddings find candidate chunks → Vibe Index pinpoints exact positions inside them.

## Query parser

```rust
use vibe_index::query_parser::parse_query;

parse_query("how does authMiddlewareChain work?")
// → [["auth", "middleware", "chain"], ["auth"], ["middleware"], ["chain"]]

parse_query("std::collections::HashMap")
// → [["std", "collections", "HashMap"], ["std"], ["collections"], ["HashMap"]]
```

Handles: camelCase, PascalCase, snake_case, kebab-case, `::` paths, generics (`Vec<String>`), acronyms.

## Unified search

`index.search(query)` is the high-level entry point:

1. Parse query → phrases (via `query_parser`)
2. Exact phrase matching on each phrase (confidence 0.95)
3. Fuzzy Levenshtein matching per significant word (confidence 0.5)
4. Deduplicate by position, sort by confidence descending

## Limitations (honest)

- **No SIMD** — tested AVX2/AVX-512 on Roaring Bitmap iteration: 64-115% slower. Roaring's internal run-compression doesn't benefit from SIMD fixed-width operations. Not planned.
- **String-based token keys** — `HashMap<String, RoaringBitmap>`. Switching to `HashMap<u32, RoaringBitmap>` with a token ID lexicon would save hash/allocation overhead per query.
- **BM25 IDF computed on-the-fly** — not precomputed. Negligible impact for small doc sets, measurable at scale.
- **Hot layer size fixed at creation** — `max_hot_tokens` is immutable after `HotColdIndex` construction.
- **Bitmap serialization uses JSON arrays** — positions stored as `[0, 1, 2, ...]` per token. Native Roaring binary format would be more compact but conflicts with serde trait methods.

## Status

- [x] Core engine (exact phrase + fuzzy Levenshtein search)
- [x] Query parser (NL → search phrases, case splitting, stop words)
- [x] BM25 candidate retrieval
- [x] Hybrid search (BM25 + Vibe Index)
- [x] Hot/Cold layer with cross-layer phrase search
- [x] Persistent storage (gzip token sequences, magic validation)
- [x] Prompt injector (context window builder)
- [x] Llama.cpp integration (tested with Qwen3VL-4B)
- [x] vLLM integration (hybrid search, context budget, output validation, confidence feedback)
- [x] Benchmarks (criterion, 9 benchmarks)
- [x] CI (build + test + bench + lint, Windows + Ubuntu)
- [x] Persistent bitmap storage (v2 format, backward compatible with v1)
- [ ] Token ID lexicon (u32 keys instead of String)

## License

MIT
