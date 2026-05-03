# vibe-index

**Sub-microsecond exact phrase matching for LLM context retrieval.**

[![CI](https://github.com/mladenpop-oss/vibe-index/actions/workflows/ci.yml/badge.svg)](https://github.com/mladenpop-oss/vibe-index/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org)

RAG pipelines stuff 8K–16K tokens into every LLM prompt — most irrelevant. Vibe Index finds **exact phrases at exact positions in microseconds**, then injects only the relevant context window. 95% less tokens, 100x faster than embedding search.

### The problem

When you build a RAG pipeline, your retriever (embeddings, BM25, etc.) finds relevant chunks — but it can't tell you **where** inside those chunks the answer lives. So you inject the entire chunk (1K–4K tokens) into the LLM prompt, even though the actual answer might be 2–3 lines.

This wastes KV cache, burns tokens, adds latency, and increases cost. Vibe Index solves this by finding **exact phrases at exact positions** — so you only inject ±50 tokens around the match instead of the whole chunk.

```
Token stream → TokenLexicon (u32 IDs) → Roaring Bitmap per token → Anchor-and-offset phrase scan
```

No embeddings. No vectors. No GPU. Just bitmaps and math.

## Performance

Measured on 50K synthetic tokens, release build, single core:

| Operation | Latency |
|-----------|---------|
| Index 50K tokens | **1.83 ms** |
| Index 10K tokens | 425 µs |
| Exact phrase (1 match) | **112 ns** |
| Exact phrase (~100 matches) | 198 µs |
| Phrase not found (early exit) | **82 ns** |
| Fuzzy 1-char typo | **4.64 µs** |
| Fuzzy 2-char typo | 67 µs |
| Fuzzy no match (early exit) | 127 ns |
| Unified NL search | 121 µs |
| Unified + typo tolerance | 36 µs |
| Hybrid BM25 + Vibe | 7–10 µs |

File-aware search with metadata (file_path, line_number, line_content):

| Operation | Latency |
|-----------|---------|
| Phrase search (10 files) | 25 µs |
| Phrase search (50 files) | 127 µs |
| Phrase search (100 files) | 250 µs |
| Phrase search (500 files) | 1.15 ms |
| Add file (10 files) | 698 µs |
| Add file (50 files) | 3.4 ms |
| Add file (100 files) | 7 ms |
| Add file (500 files) | 36 ms |
| Persist + reload (10 files) | 4 ms |
| Persist + reload (100 files) | 150 µs |

## Why
Basically it's a superfast exact phrase search for code. You index your files, then it finds "fn authenticate" at exactly line 42 in 111 nanoseconds.

The whole point is regular retrievers (embeddings, BM25) find the right chunk but can't tell you where inside that chunk the answer is. So you waste tokens stuffing the whole chunk into the LLM prompt. Vibe Index finds the exact line, so you only inject what you actually need. 

Vibe Index is a sub-microsecond exact phrase search engine designed for LLM context retrieval. Index your source code, then find `"fn authenticate"` at exactly line 42 in 112 nanoseconds.

Regular retrievers (embeddings, BM25) find the right chunk but can't tell you where inside that chunk the answer lives. This forces you to inject the entire chunk (1K–4K tokens) into the LLM prompt, wasting tokens, KV cache, and inference time. Vibe Index finds the exact line so you only inject ±50 tokens around the match.

Every LLM query pays for context it doesn't need. A 7B model processes ~1.5KB per token in KV cache. Injecting 4K tokens of context instead of 300 relevant tokens wastes 5.5GB of VRAM and adds 30+ seconds of inference time.

Vibe Index solves this by:

1. **Exact positional lookup** — find `fn authenticate` at line 42, not "somewhere in chunk 3"
2. **File-aware search** — results include `file_path`, `line_number`, and `line_content`
3. **Minimal context injection** — inject ±50 tokens around the match, not 1024-token chunks
4. **Typo tolerance** — "proces" still finds "process" in 2µs via bigram prefiltering
5. **Hybrid with BM25** — BM25 finds relevant documents, Vibe pinpoints exact lines within them
6. **Incremental updates** — `update_file()` changes only modified files using content hashing
7. **Ranked results** — confidence scoring with file size weighting and snippet highlighting

## Quick Start

```bash
cargo add vibe-index
```

```rust
use vibe_index::VibeIndex;

let mut index = VibeIndex::new();

// Index a file with metadata (preferred for source code)
index.add_file("src/auth.rs", r#"
fn authenticate(user: &str) -> Result<(), Error> {
    Ok(())
}
"#);

// Or index raw text (legacy, no file tracking)
for token in &["fn", "main", "(", ")"] {
    index.add_token(token);
}

// Exact phrase search — returns file_path, line_number, line_content
let results = index.phrase_search(&["fn".into(), "authenticate".into()]);
// → [MatchResult { position: 0, file_path: "src/auth.rs", line_number: 2, ... }]

// Unified search: NL → phrase + fuzzy → ranked results
let results = index.search("where is the authenticate function");
// → Exact matches (confidence 0.95) + fuzzy matches (confidence 0.5)

// Hybrid: BM25 candidates → Vibe exact positions
use vibe_index::hybrid_search::HybridSearcher;
let hybrid = HybridSearcher::new(top_k: 3);
let results = hybrid.search("how does authentication work");
```

### Feature Flags

The `llama_cpp` and `vllm` modules are optional. Enable them via Cargo features:

```bash
# Core only (no extra dependencies)
cargo add vibe-index

# With llama.cpp integration
cargo add vibe-index --features llama-cpp

# With vLLM integration
cargo add vibe-index --features vllm

# All features
cargo add vibe-index --features all
```

## How It Works

### Phrase Search (anchor-and-offset)

```
query: ["fn", "authenticate", "("]

1. Resolve each token → u32 ID via lexicon
2. Get Roaring Bitmap for each ID
3. Pick smallest bitmap as anchor (fewest iterations)
4. For each position P in anchor bitmap:
   - Check if sibling tokens exist at P + offset
   - All match → exact phrase found at P
```

Complexity: O(min_cardinality × phrase_length) instead of O(N) linear scan.

### Fuzzy Search (bigram prefiltering)

```
query: "proces" (missing "s")
max_distance: 1

1. Extract bigrams from query: ["pr", "ro", "oc", "ce", "es"]
2. Look up each bigram → candidate token IDs
3. Deduplicate → typically 3% of all tokens
4. Length filter: skip tokens where |len(query) - len(token)| > max_distance
5. Compute Levenshtein only for remaining candidates
```

Without prefiltering: Levenshtein against all 672 unique tokens. With prefiltering: Levenshtein against ~20 tokens. **97% fewer computations.**

### Comparison

| Approach | Precision | Latency | Memory (50K tokens) |
|----------|-----------|---------|---------------------|
| **Vibe Index** | Exact token position | 70ns – 120µs | ~0.5 MB |
| **BM25** | Document-level match | 50-500 µs | ~2 MB |
| **FAISS (embeddings)** | Semantic ~0.85 similarity | 5-20 ms | ~20 MB |
| **Tantivy** | Document-level match | 50-200 µs | ~3 MB |

Vibe Index is 100-1000x faster than embedding search and provides exact positions (not document-level matches). It does not replace semantic search — it complements it.

**Hybrid pattern:** Embeddings/BM25 find candidate chunks → Vibe Index pinpoints exact positions within them.

## Modules

| Module | Purpose |
|--------|---------|
| `VibeIndex` | Core engine: index, phrase/fuzzy/unified search, incremental `update_file()` |
| `TokenLexicon` | u32 ID ↔ String mapping + bigram index for fast fuzzy prefiltering |
| `file_index` | File metadata: paths, content, line offsets, token-to-line mapping, FNV-1a hashing |
| `query_parser` | NL → search phrases: splits camelCase, snake_case, `::` paths, strips stop words |
| `bm25` | Lightweight BM25 scorer for document-level candidate ranking |
| `hybrid_search` | BM25 candidates → Vibe Index exact position validation |
| `hot_cold` | In-memory hot buffer + disk-backed cold segments with configurable context window |
| `persistent_storage` | Gzip-compressed token sequences + serialized bitmaps + file metadata |
| `prompt_injector` | Context window builder: search → filter → extract windows → build prompt |
| `llama_cpp` | Full pipeline: index → search → build prompt → llama.cpp completion (optional) |
| `vllm` | Hybrid search + context budget + output validation + confidence feedback (optional) |
| `mcp_server` | MCP (Model Context Protocol) server for LLM tool integration |

## Configurable Context Windows

Both the hot/cold index and prompt injector support configurable context window sizes:

```rust
// Hot/Cold index with custom context window
let mut hc = HotColdIndex::new("./data", 10000);
hc.context_window_size = 20; // ±20 tokens around matches

// Prompt injector with custom context window
let injector = PromptInjector::with_context_window(
    max_context_tokens: 500,
    min_confidence: 0.5,
    window_size: 10, // ±10 tokens around matches
);
```

## MCP Server

Expose Vibe Index as an MCP tool for LLM applications (LM Studio, Ollama, Claude Desktop, OpenCode, etc.).

### Python MCP Server

Python implementation using the official `mcp` package. Ideal for OpenCode Desktop integration.

**Install:**
```bash
pip install mcp
```

**Run:**
```bash
python mcp_server.py
```

### Available Tools

| Tool | Description |
|------|-------------|
| `index_text` | Index raw text content into the Vibe Index |
| `index_file` | Index a file with path metadata (preserves file boundaries, enables line number lookup) |
| `phrase_search` | Exact phrase search with positional results — ranked by confidence |
| `fuzzy_search` | Typo-tolerant search (Levenshtein distance) — ranked by confidence |
| `search` | Unified natural language search — ranked by confidence |
| `get_file_content` | Get the full content of an indexed file by path |
| `get_stats` | Index statistics (positions, tokens, memory, file count) |
| `clear_index` | Reset the index to empty state |

### OpenCode Desktop Configuration

Add to `~/.config/opencode/opencode.jsonc` or `%APPDATA%\opencode\opencode.jsonc`:

```json
{
  "mcp": {
    "vibe-index": {
      "type": "local",
      "command": ["python", "path/to/mcp_server.py"],
      "enabled": true,
      "timeout": 10000
    }
  }
}
```

### LM Studio / Claude Desktop Configuration

```json
{
  "mcpServers": {
    "vibe-index": {
      "command": "python",
      "args": ["path/to/mcp_server.py"]
    }
  }
}
```

### Example Usage

```json
// Index a file with metadata (preferred)
{"name": "index_file", "arguments": {"file_path": "src/main.rs", "content": "fn main() { let x = 42; }"}}

// Index raw text (legacy)
{"name": "index_text", "arguments": {"text": "fn main() { let x = 42; }"}}

// Search
{"name": "search", "arguments": {"query": "where is the main function"}}

// Phrase search
{"name": "phrase_search", "arguments": {"phrase": "fn main"}}

// Fuzzy search (1 typo tolerance)
{"name": "fuzzy_search", "arguments": {"query": "mian", "max_distance": 1}}

// Get file content
{"name": "get_file_content", "arguments": {"file_path": "src/auth.rs"}}
```

## Example

Index a real codebase and search it:

```bash
cargo run --example real_codebase_search -- <directory> <query>
```

Example:
```bash
cargo run --example real_codebase_search -- src "fn main"
```

This walks all `.rs` files in the directory, indexes them with file metadata, and performs unified search (phrase + fuzzy) with ranked results.

## Recent Updates

### v0.1.3 — Feature Flags, Configurable Windows & Documentation

- **Feature flags** — `llama-cpp` and `vllm` modules are now optional. Use `--features all` to enable all LLM integrations, or select individual features. Core indexing works with zero extra dependencies.
- **Configurable context window** — `PromptInjector::with_context_window()` and `HotColdIndex.context_window_size` allow tuning the ±N token context around matches.
- **Full rustdoc coverage** — all public types and methods now have comprehensive documentation with runnable examples (7 doc tests).
- **New tests** — 6 tests for `prompt_injector`, 8 tests for `vllm` module. Total: 73 unit tests + 7 doc tests.
- **Confidence scoring fixed** — file size weighting now works correctly (removed erroneous 1.0 cap).
- **Cleaned up** — removed hardcoded Windows paths from benchmarks, replaced custom `min()` with `usize::min`, removed dead `.bak` files.

### v0.1.2 — Incremental Indexing & Search Enhancements

- **Incremental file indexing** — `update_file()` updates changed files without full re-index. Uses FNV-1a content hashing for change detection.
- **Snippet highlighting** — matched tokens are wrapped in `**bold**` markers for easy visual identification.
- **File size weighting** — larger files get a logarithmic confidence boost (diminishing returns).
- **Fuzzy search ranking** — fuzzy matches now benefit from file size weighting, consistent with phrase search.
- **`real_codebase_search` example** — new example that indexes a real codebase and searches it.

### v0.1.1 — Performance & UX Improvements

- **Binary search for file lookups** — `O(log n)` lookup instead of `O(n)` linear scan. 2.5x faster at 100+ files.
- **Relevance ranking** — search results sorted by confidence (highest first), grouped by file.
- **`get_file_content` MCP tool** — retrieve full content of indexed files by path.

## Testing

The project maintains comprehensive test coverage with 73 unit tests and 7 doc tests:

- **Core indexing**: add_token, add_file, total_positions, unique_tokens
- **Phrase search**: exact matches, not found, multiple matches, with file info
- **Fuzzy search**: typo tolerance, bigram prefiltering
- **Unified search**: phrase match, fuzzy match, combined, empty query
- **File tracking**: file segments, line offsets, token-line mapping, binary search, content hashing
- **Incremental updates**: no-change detection, content change, new file, range shifting
- **Persistence**: save/load, batch import, legacy format migration, file metadata
- **Hot/Cold**: flushes, multiple flushes, stats, cross-layer search, persistence
- **Query parser**: camelCase, snake_case, kebab-case, paths, generics, stop words
- **Prompt injector**: basic search, confidence filtering, context windows, empty context
- **vLLM integration**: context validation, output validation, confidence feedback, stats

Run tests:
```bash
cargo test
```

Run benchmarks:
```bash
cargo bench
```

## Author

**Mladen Popović**

## Limitations

- **No semantic search** — "login" ≠ "authenticate" to Vibe Index. Use embeddings for conceptual matching, then Vibe for exact positioning.
- **BM25 IDF computed on-the-fly** — negligible for small doc sets, measurable at scale.
- **Hot layer size immutable** — `max_hot_tokens` fixed at `HotColdIndex` creation.
- **No SIMD** — tested AVX2/AVX-512 on Roaring iteration: 64-115% slower. Run-compression doesn't benefit from fixed-width SIMD ops.

## License

MIT
