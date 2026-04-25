/// SIMD-accelerated phrase search using array-based parallel processing.
///
/// Uses fixed-size arrays that the compiler can auto-vectorize with AVX2/AVX-512.
/// No nightly features required - relies on LLVM auto-vectorization in release builds.
///
/// The core idea: process 8 anchor positions at once, compute expected offsets
/// for all query tokens in parallel, then validate via binary search.
pub struct SimdPhraseSearch;

impl SimdPhraseSearch {
    /// Number of positions processed per batch (matches SIMD register width).
    const BATCH_SIZE: usize = 8;

    /// Perform SIMD-accelerated phrase search.
    ///
    /// # Arguments
    /// * `anchor_positions` - Sorted positions from the smallest (anchor) bitmap
    /// * `anchor_query_idx` - Index of the anchor token in the query
    /// * `token_bitmaps` - All token position arrays with their query indices
    ///
    /// # Returns
    /// Sorted vector of matching start positions.
    #[inline(never)]
    pub fn search(
        anchor_positions: &[u32],
        anchor_query_idx: usize,
        token_bitmaps: &[(usize, &[u32])],
    ) -> Vec<u32> {
        if anchor_positions.is_empty() || token_bitmaps.is_empty() {
            return Vec::new();
        }

        let mut matches = Vec::new();

        // Process in batches for auto-vectorization
        let batch_size = Self::BATCH_SIZE;
        let chunks = anchor_positions.chunks(batch_size);

        for chunk in chunks {
            let len = chunk.len();

            // Load into fixed-size arrays for vectorization
            let mut anchor_batch = [0i64; Self::BATCH_SIZE];
            for i in 0..len {
                anchor_batch[i] = chunk[i] as i64;
            }

            // Track which lanes are still valid
            let mut valid = [true; Self::BATCH_SIZE];

            // For each token bitmap, validate positions in parallel
            for (query_idx, (_, positions)) in token_bitmaps.iter().enumerate() {
                // Offset for this token relative to anchor:
                // expected_pos = anchor_pos + (query_idx - anchor_query_idx)
                let token_offset = query_idx as i64 - anchor_query_idx as i64;

                // Compute expected positions: anchor + offset (vectorizable)
                let mut expected = [0i64; Self::BATCH_SIZE];
                for i in 0..len {
                    expected[i] = anchor_batch[i] + token_offset;
                }

                // Validate each expected position against the bitmap
                for i in 0..len {
                    if valid[i] {
                        let exp = expected[i];
                        if exp < 0 {
                            valid[i] = false;
                        } else {
                            valid[i] = Self::binary_search_u32(positions, exp as u32);
                        }
                    }
                }

                // Early exit if all lanes are invalid
                let any_valid = (0..len).any(|i| valid[i]);
                if !any_valid {
                    break;
                }
            }

            // Collect valid matches
            // first_pos = anchor_pos - anchor_query_idx
            let first_offset = anchor_query_idx as i64;
            for i in 0..len {
                if valid[i] {
                    let first_pos = anchor_batch[i] - first_offset;
                    if first_pos >= 0 {
                        matches.push(first_pos as u32);
                    }
                }
            }
        }

        matches.sort();
        matches.dedup();
        matches
    }

    /// Binary search for a u32 value in a sorted slice.
    #[inline]
    fn binary_search_u32(slice: &[u32], target: u32) -> bool {
        slice.binary_search(&target).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_search_exact_match() {
        let fn_pos = vec![0, 2, 4];
        let main_pos = vec![1, 3, 5];
        let token_bitmaps = [(0, fn_pos.as_slice()), (1, main_pos.as_slice())];

        let result = SimdPhraseSearch::search(&fn_pos, 0, &token_bitmaps);
        assert_eq!(result, vec![0, 2, 4]);
    }

    #[test]
    fn test_simd_search_no_match() {
        let a_pos = vec![0, 5, 10];
        let b_pos = vec![2, 7, 12];
        let token_bitmaps = [(0, a_pos.as_slice()), (1, b_pos.as_slice())];

        let result = SimdPhraseSearch::search(&a_pos, 0, &token_bitmaps);
        assert!(result.is_empty());
    }

    #[test]
    fn test_simd_search_partial_match() {
        let fn_pos = vec![0, 5, 10];
        let main_pos = vec![1, 20];
        let token_bitmaps = [(0, fn_pos.as_slice()), (1, main_pos.as_slice())];

        let result = SimdPhraseSearch::search(&fn_pos, 0, &token_bitmaps);
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn test_simd_search_anchor_not_first() {
        // "hello"=[0,10], "world"=[1,11,20], "foo"=[2,21]
        // "hello world foo" matches at P=0: hello@0, world@1, foo@2
        // P=10 fails: hello@10, world@11, foo@12 (foo@12 doesn't exist)
        let hello_pos = vec![0, 10];
        let world_pos = vec![1, 11, 20];
        let foo_pos = vec![2, 21];
        let token_bitmaps = [
            (0, hello_pos.as_slice()),
            (1, world_pos.as_slice()),
            (2, foo_pos.as_slice()),
        ];

        let result = SimdPhraseSearch::search(&world_pos, 1, &token_bitmaps);
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn test_simd_search_large_chunk() {
        let mut a_pos = Vec::new();
        let mut b_pos = Vec::new();
        for i in 0..20 {
            a_pos.push(i * 2);
            b_pos.push(i * 2 + 1);
        }
        let token_bitmaps = [(0, a_pos.as_slice()), (1, b_pos.as_slice())];

        let result = SimdPhraseSearch::search(&a_pos, 0, &token_bitmaps);
        assert_eq!(result.len(), 20);
    }

    #[test]
    fn test_simd_search_empty_anchor() {
        let token_bitmaps: [(usize, &[u32]); 0] = [];
        let result = SimdPhraseSearch::search(&[], 0, &token_bitmaps);
        assert!(result.is_empty());
    }

    #[test]
    fn test_simd_search_many_occurrences() {
        // Simulate a common word appearing many times
        let mut pos_a = Vec::new();
        let mut pos_b = Vec::new();
        for i in 0..100 {
            pos_a.push(i * 10);
            pos_b.push(i * 10 + 1);
        }

        let token_bitmaps = [(0, pos_a.as_slice()), (1, pos_b.as_slice())];
        let result = SimdPhraseSearch::search(&pos_a, 0, &token_bitmaps);
        assert_eq!(result.len(), 100, "All 100 pairs should match");
        assert_eq!(result[0], 0);
        assert_eq!(result[99], 990);
    }
}
