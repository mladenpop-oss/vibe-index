"""Vibe Index MCP Server - Python implementation with file tracking"""
import sys
import json
import re
import asyncio
from mcp.server.fastmcp import FastMCP

mcp = FastMCP("vibe-index")


class FileSegment:
    """Represents a single indexed file with line tracking"""
    def __init__(self, path: str, content: str, token_start: int, token_end: int):
        self.path = path
        self.content = content
        self.token_start = token_start
        self.token_end = token_end
        self.token_count = token_end - token_start
        self.line_offsets = self._compute_line_offsets(content)
    
    def _compute_line_offsets(self, content: str) -> list:
        offsets = [0]
        for i, byte in enumerate(content):
            if byte == '\n':
                offsets.append(i + 1)
        return offsets
    
    def byte_offset_to_line(self, byte_offset: int) -> int:
        import bisect
        idx = bisect.bisect_right(self.line_offsets, byte_offset) - 1
        if idx < 0:
            return 1
        # If we're exactly on a line boundary (start of line), return that line
        if byte_offset > self.line_offsets[idx]:
            return idx + 1
        return idx + 1
    
    def token_to_line(self, global_token_pos: int) -> tuple:
        if global_token_pos < self.token_start or global_token_pos >= self.token_end:
            return None
        local_token_pos = global_token_pos - self.token_start
        total_tokens = self.token_count
        total_bytes = len(self.content)
        if total_tokens == 0:
            return None
        estimated_byte_pos = int(local_token_pos / total_tokens * total_bytes)
        line_number = self.byte_offset_to_line(estimated_byte_pos)
        line_content = self.get_line_content(line_number)
        return (line_number, line_content)
    
    def get_line_content(self, line_number: int) -> str:
        if line_number <= 0 or line_number > len(self.line_offsets):
            return ""
        start = self.line_offsets[line_number - 1]
        end = self.line_offsets[line_number] if line_number < len(self.line_offsets) else len(self.content)
        return self.content[start:end].rstrip('\n')
    
    def contains_token(self, global_pos: int) -> bool:
        return global_pos >= self.token_start and global_pos < self.token_end


class VibeIndex:
    def __init__(self):
        self.lexicon: dict[str, int] = {}
        self.id_to_token: dict[int, str] = {}
        self.token_positions: dict[int, list[int]] = {}
        self.token_sequence: list[int] = []
        self.bigram_index: dict[str, set[int]] = {}
        self._next_id = 0
        self.files: list[FileSegment] = []
    
    def add_token(self, token: str) -> None:
        token_lower = token.lower()
        if token_lower not in self.lexicon:
            self.lexicon[token_lower] = self._next_id
            self.id_to_token[self._next_id] = token_lower
            self.token_positions[self._next_id] = []
            self._next_id += 1
        
        token_id = self.lexicon[token_lower]
        position = len(self.token_sequence)
        self.token_sequence.append(token_id)
        self.token_positions[token_id].append(position)
        
        for i in range(len(token_lower) - 1):
            bigram = token_lower[i:i+2]
            if bigram not in self.bigram_index:
                self.bigram_index[bigram] = set()
            self.bigram_index[bigram].add(token_id)
    
    def add_file(self, file_path: str, content: str) -> None:
        token_start = len(self.token_sequence)
        tokens = re.split(r'[^a-zA-Z0-9_]+', content)
        tokens = [t for t in tokens if t]
        
        for token in tokens:
            self.add_token(token)
        
        token_end = len(self.token_sequence)
        file_segment = FileSegment(file_path, content, token_start, token_end)
        self.files.append(file_segment)
    
    def get_file_info(self, token_pos: int) -> tuple:
        for file_seg in self.files:
            if file_seg.contains_token(token_pos):
                line_info = file_seg.token_to_line(token_pos)
                if line_info:
                    return (file_seg.path, line_info[0], line_info[1])
        return (None, None, None)
    
    def phrase_search(self, tokens: list[str]) -> list[dict]:
        if not tokens:
            return []
        
        token_ids = []
        for t in tokens:
            tid = self.lexicon.get(t.lower())
            if tid is None:
                return []
            token_ids.append(tid)
        
        results = []
        anchor_ids = self.token_positions[token_ids[0]]
        
        for pos in anchor_ids:
            match = True
            for offset, target_id in enumerate(token_ids[1:]):
                if (pos + offset + 1) not in self.token_positions.get(target_id, []):
                    match = False
                    break
            
            if match:
                context = self._get_context(pos)
                file_path, line_num, line_content = self.get_file_info(pos)
                results.append({
                    "position": pos,
                    "confidence": 1.0,
                    "context": context,
                    "file_path": file_path,
                    "line_number": line_num,
                    "line_content": line_content,
                })
        
        return results
    
    def fuzzy_search(self, query: str, max_distance: int = 1) -> list[dict]:
        query_lower = query.lower()
        query_bigrams = set(query_lower[i:i+2] for i in range(len(query_lower) - 1))
        
        if not query_bigrams:
            return []
        
        candidate_ids = set()
        for bigram in query_bigrams:
            candidate_ids.update(self.bigram_index.get(bigram, set()))
        
        results = []
        for tid in candidate_ids:
            token = self.id_to_token[tid]
            if abs(len(token) - len(query_lower)) > max_distance:
                continue
            
            dist = self._levenshtein(query_lower, token)
            if dist <= max_distance:
                confidence = 1.0 - (dist / max(len(query_lower), len(token)))
                pos = self.token_positions[tid][0] if self.token_positions[tid] else 0
                context = self._get_context(pos)
                file_path, line_num, line_content = self.get_file_info(pos)
                results.append({
                    "position": pos,
                    "confidence": confidence,
                    "context": context,
                    "matched_token": token,
                    "file_path": file_path,
                    "line_number": line_num,
                    "line_content": line_content,
                })
        
        return sorted(results, key=lambda x: x["confidence"], reverse=True)
    
    def search(self, query: str) -> list[dict]:
        tokens = re.split(r'[^a-zA-Z0-9_]+', query.lower())
        tokens = [t for t in tokens if t]
        
        stop_words = {'where', 'is', 'the', 'how', 'does', 'what', 'when', 'why', 'to', 'of', 'in', 'for', 'with', 'on', 'at', 'by'}
        search_tokens = [t for t in tokens if t not in stop_words]
        
        if not search_tokens:
            search_tokens = tokens
        
        results = []
        
        for i in range(1, len(search_tokens) + 1):
            for j in range(len(search_tokens) - i + 1):
                phrase = search_tokens[j:j+i]
                matches = self.phrase_search(phrase)
                for m in matches:
                    m["confidence"] = min(1.0, 0.95 + (len(phrase) * 0.02))
                    results.append(m)
        
        for token in search_tokens:
            matches = self.fuzzy_search(token, max_distance=1)
            for m in matches:
                m["confidence"] = max(m["confidence"], 0.5)
                results.append(m)
        
        seen = set()
        unique = []
        for r in sorted(results, key=lambda x: x["confidence"], reverse=True):
            key = (r["position"], r["context"][:50])
            if key not in seen:
                seen.add(key)
                unique.append(r)
        
        return unique[:20]
    
    def _levenshtein(self, s1: str, s2: str) -> int:
        if len(s1) < len(s2):
            return self._levenshtein(s2, s1)
        
        if len(s2) == 0:
            return len(s1)
        
        prev_row = range(len(s2) + 1)
        for i, c1 in enumerate(s1):
            curr_row = [i + 1]
            for j, c2 in enumerate(s2):
                insertions = prev_row[j + 1] + 1
                deletions = curr_row[j] + 1
                substitutions = prev_row[j] + (c1 != c2)
                curr_row.append(min(insertions, deletions, substitutions))
            prev_row = curr_row
        
        return prev_row[-1]
    
    def _get_context(self, position: int, window: int = 50) -> str:
        start = max(0, position - window)
        end = min(len(self.token_sequence), position + window)
        
        tokens = [self.id_to_token[tid] for tid in self.token_sequence[start:end]]
        context = " ".join(tokens)
        
        if start > 0:
            context = "... " + context
        if end < len(self.token_sequence):
            context = context + " ..."
        
        return context
    
    def total_positions(self) -> int:
        return len(self.token_sequence)
    
    def unique_tokens(self) -> int:
        return len(self.lexicon)
    
    def estimated_memory_bytes(self) -> int:
        return sys.getsizeof(self.token_sequence) + sys.getsizeof(self.lexicon) + sys.getsizeof(self.token_positions)


index = VibeIndex()


@mcp.tool()
async def index_text(text: str) -> str:
    tokens = re.split(r'[^a-zA-Z0-9_]+', text)
    tokens = [t for t in tokens if t]
    
    for token in tokens:
        index.add_token(token)
    
    return f"Indexed {len(tokens)} tokens ({index.unique_tokens()} unique). Total positions: {index.total_positions()}"


@mcp.tool()
async def index_file(file_path: str, content: str) -> str:
    token_count = len([t for t in re.split(r'[^a-zA-Z0-9_]+', content) if t])
    index.add_file(file_path, content)
    return f"Indexed file '{file_path}': {token_count} tokens ({index.unique_tokens()} unique tokens total, {index.total_positions()} positions, {len(index.files)} files indexed)"


@mcp.tool()
async def phrase_search(phrase: str) -> str:
    tokens = re.split(r'[^a-zA-Z0-9_]+', phrase)
    tokens = [t for t in tokens if t]
    
    results = index.phrase_search(tokens)
    
    if not results:
        return "No matches found."
    
    # Group by file
    grouped = {}
    for r in results:
        key = r.get("file_path") or "(unknown)"
        if key not in grouped:
            grouped[key] = []
        grouped[key].append(r)
    
    lines = []
    for file, matches in grouped.items():
        lines.append(f"=== {file} ({len(matches)} matches) ===")
        for r in matches:
            if r.get("line_number") and r.get("line_content"):
                line_info = f"line {r['line_number']}: {r['line_content'].strip()}"
            else:
                line_info = f"POS {r['position']}"
            lines.append(f"  [POS {r['position']}] conf={r['confidence']:.2f} | {line_info}")
        lines.append("")
    
    return "\n".join(lines)


@mcp.tool()
async def fuzzy_search(query: str, max_distance: int = 1) -> str:
    results = index.fuzzy_search(query, max_distance)
    
    if not results:
        return "No fuzzy matches found."
    
    # Group by file
    grouped = {}
    for r in results:
        key = r.get("file_path") or "(unknown)"
        if key not in grouped:
            grouped[key] = []
        grouped[key].append(r)
    
    lines = []
    for file, matches in grouped.items():
        lines.append(f"=== {file} ({len(matches)} matches) ===")
        for r in matches:
            if r.get("line_number") and r.get("line_content"):
                line_info = f"line {r['line_number']}: {r['line_content'].strip()}"
            else:
                line_info = f"POS {r['position']}"
            lines.append(f"  [POS {r['position']}] conf={r['confidence']:.2f} | {line_info} (matched: {r.get('matched_token', '')})")
        lines.append("")
    
    return "\n".join(lines)


@mcp.tool()
async def search(query: str) -> str:
    results = index.search(query)
    
    if not results:
        return "No matches found."
    
    # Group by file
    grouped = {}
    for r in results:
        key = r.get("file_path") or "(unknown)"
        if key not in grouped:
            grouped[key] = []
        grouped[key].append(r)
    
    lines = []
    lines.append(f"Found {len(results)} matches across {len(grouped)} files:\n")
    for file, matches in grouped.items():
        lines.append(f"=== {file} ({len(matches)} matches) ===")
        for r in matches[:5]:
            if r.get("line_number") and r.get("line_content"):
                line_info = f"line {r['line_number']}: {r['line_content'].strip()}"
            else:
                line_info = f"POS {r['position']}"
            lines.append(f"  [POS {r['position']}] conf={r['confidence']:.2f} | {line_info}")
        if len(matches) > 5:
            lines.append(f"  ... and {len(matches) - 5} more matches")
        lines.append("")
    
    return "\n".join(lines)


@mcp.tool()
async def get_stats() -> str:
    mem_kb = index.estimated_memory_bytes() / 1024
    file_list = "\n".join(f"  - {f.path}: {f.token_count} tokens" for f in index.files)
    return f"Total positions: {index.total_positions()}\nUnique tokens: {index.unique_tokens()}\nMemory: {mem_kb:.2f} KB ({index.estimated_memory_bytes()} bytes)\nFiles indexed: {len(index.files)}\n\n{file_list}\nTotal indexed tokens: {sum(f.token_count for f in index.files)}"


@mcp.tool()
async def clear_index() -> str:
    index.__init__()
    return "Index cleared."


if __name__ == "__main__":
    mcp.run()
