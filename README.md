# vibe-index

**Sub-microsecond exact phrase matching for LLM context retrieval.**

[![CI](https://github.com/mladenpop-oss/vibe-index/actions/workflows/ci.yml/badge.svg)](https://github.com/mladenpop-oss/vibe-index/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org)

RAG pipelines stuff 8K–16K tokens into every LLM prompt — most irrelevant. Vibe Index finds **exact phrases at exact positions in microseconds**, then injects only the relevant context window. 95% less tokens, 100x faster than embedding search.

```
Token stream → TokenLexicon (u32 IDs) → Roaring Bitmap per token → Anchor-and-offset phrase scan
```

No embeddings. No vectors. No GPU. Just bitmaps and math.

## Performance

Measured on 50K synthetic tokens, release build, single core:

| Operation | Latency |
|-----------|---------|
| Index 50K tokens | 1.84 ms |
| Exact phrase (1 match) | **111 ns** |
| Exact phrase (~100 matches) | 188 µs |
| Phrase not found (early exit) | 80 ns |
| Fuzzy 1-char typo | **4.47 µs** |
| Fuzzy 2-char typo | 66.8 µs |
| Fuzzy no match (early exit) | 116 ns |
| Unified NL search | 120 µs |
| Unified + typo tolerance | 35.8 µs |
| Hybrid BM25 + Vibe | 6.9 µs |

Real codebase (vibe-index itself, 15.8K tokens, 0.14 MB memory):

| Operation | Latency |
|-----------|---------|
| Index 10 .rs files | 14.2 ms |
| `pub fn new` (3-word phrase) | 83.9 µs |
| `impl Default for` (exact) | 7.5 µs |
| "phrase search function" (NL) | 714 µs |
| "pharse searsh" (fuzzy, 2 typos) | 490 µs |

File-aware search with metadata (file_path, line_number, line_content):

| Operation | Latency |
|-----------|---------|
| Phrase search (10 files) | 21.5 µs |
| Phrase search (100 files) | 239 µs |
| Phrase search (500 files) | 1.12 ms |
| Add file (10 files) | 677 µs |
| Add file (500 files) | 34.5 ms |
| Persist + reload (100 files) | 128 µs |

## Why

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
| `hot_cold` | In-memory hot buffer + disk-backed cold segments |
| `persistent_storage` | Gzip-compressed token sequences + serialized bitmaps + file metadata |
| `prompt_injector` | Context window builder: search → filter → extract windows → build prompt |
| `llama_cpp` | Full pipeline: index → search → build prompt → llama.cpp completion |
| `vllm` | Hybrid search + context budget + output validation + confidence feedback |
| `mcp_server` | MCP (Model Context Protocol) server for LLM tool integration |

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

## Author

**Mladen Popović** 

## Limitations

- **No semantic search** — "login" ≠ "authenticate" to Vibe Index. Use embeddings for conceptual matching, then Vibe for exact positioning.
- **BM25 IDF computed on-the-fly** — negligible for small doc sets, measurable at scale.
- **Hot layer size immutable** — `max_hot_tokens` fixed at `HotColdIndex` creation.
- **No SIMD** — tested AVX2/AVX-512 on Roaring iteration: 64-115% slower. Run-compression doesn't benefit from fixed-width SIMD ops.

## License

MIT
