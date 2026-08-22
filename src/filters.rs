//! Reversible byte-stream transforms (`JOURNAL` S1-A2, M1 port).
//!
//! Filters trade the input for a differently-shaped byte stream that a
//! downstream model can predict more cheaply — never a smaller one on their
//! own. Trial selection (`JOURNAL` S1-A1), not a static rule, decides when
//! to apply a filter; each submodule here only provides the reversible
//! transform itself. The `pick_filters` trial-selection heuristic and
//! wiring behind a [`crate::Method`] variant are follow-up slices
//! (`JOURNAL` S2-D2). Remaining filter kinds (BCJ, base64-unwrap, reverse):
//! same DEBT entry.

/// Fixed-stride delta filter.
///
/// Wins on data with fixed-width records where corresponding columns of
/// adjacent rows are numerically close (for example interleaved
/// multi-channel audio samples); `JOURNAL` S1-R1 shows the same transform
/// loses on text, where the numeric difference of letters is *more*
/// scattered than the letters themselves.
pub mod delta {
    use std::num::NonZeroUsize;

    /// Replaces each byte at index `i >= stride` with its wrapping
    /// difference from the byte at `i - stride`; the first `stride` bytes
    /// are left as-is.
    ///
    /// Reversible by [`decode`] with the same `stride`. `stride` is
    /// [`NonZeroUsize`] because a zero stride would subtract every byte
    /// from itself, zeroing the data instead of transforming it reversibly
    /// — the type rules out that misuse instead of a runtime check.
    #[must_use]
    pub fn encode(data: &[u8], stride: NonZeroUsize) -> Vec<u8> {
        let stride = stride.get();
        let mut out = data.to_vec();
        for i in stride..out.len() {
            out[i] = data[i].wrapping_sub(data[i - stride]);
        }
        out
    }

    /// Inverts [`encode`] with the same `stride`.
    #[must_use]
    pub fn decode(data: &[u8], stride: NonZeroUsize) -> Vec<u8> {
        let stride = stride.get();
        let mut out = data.to_vec();
        for i in stride..out.len() {
            out[i] = out[i].wrapping_add(out[i - stride]);
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn nz(v: usize) -> NonZeroUsize {
            NonZeroUsize::new(v).unwrap()
        }

        #[test]
        fn roundtrip_empty() {
            let s = nz(1);
            assert_eq!(decode(&encode(&[], s), s), Vec::<u8>::new());
        }

        #[test]
        fn roundtrip_single_byte() {
            let s = nz(1);
            assert_eq!(decode(&encode(&[42], s), s), vec![42]);
        }

        #[test]
        fn roundtrip_shorter_than_stride() {
            // stride longer than the data: every byte is left untouched,
            // both directions are the identity.
            let data = vec![1, 2, 3];
            let s = nz(8);
            assert_eq!(encode(&data, s), data);
            assert_eq!(decode(&data, s), data);
        }

        #[test]
        fn roundtrip_various_strides() {
            let data: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
            for stride in [1usize, 2, 3, 4, 8, 16, 96, 255, 1000, 1001] {
                let s = nz(stride);
                let encoded = encode(&data, s);
                assert_eq!(decode(&encoded, s), data, "stride {stride}");
            }
        }

        #[test]
        fn encode_is_identity_for_first_stride_bytes() {
            let data = vec![10, 20, 30, 40, 50];
            let s = nz(2);
            let encoded = encode(&data, s);
            assert_eq!(&encoded[..2], &data[..2]);
        }

        #[test]
        fn wrapping_arithmetic_survives_overflow() {
            // Constructed so intermediate subtraction underflows u8; must
            // still round-trip losslessly via wrapping ops.
            let data = vec![0u8, 255, 1, 254, 2];
            let s = nz(1);
            assert_eq!(decode(&encode(&data, s), s), data);
        }
    }
}

/// Row-major to column-major byte reordering.
///
/// Treats `data` as rows of `columns` bytes each (the last row possibly
/// short) and rewrites it column by column: all bytes at column 0, then all
/// bytes at column 1, and so on. Wins on data with a fixed record width
/// where a column carries its own regularity across rows (`JOURNAL` S1-A2's
/// x-ray dataset: −0.47 b/B). Distinct from [`delta`], which predicts a
/// column's *next* value from the previous row; this filter regroups
/// columns so a downstream model with only short-range context can see
/// that regularity at all.
pub mod transpose {
    use std::num::NonZeroUsize;

    /// Rewrites `data`, interpreted as rows of `columns` bytes, column by
    /// column.
    ///
    /// Reversible by [`decode`] with the same `columns`. `columns` is
    /// [`NonZeroUsize`] because a zero column count has no rows to
    /// transpose — the type rules out that empty case instead of a
    /// runtime check.
    #[must_use]
    pub fn encode(data: &[u8], columns: NonZeroUsize) -> Vec<u8> {
        let columns = columns.get();
        let n = data.len();
        let mut out = Vec::with_capacity(n);
        for start in 0..columns {
            let mut i = start;
            while i < n {
                out.push(data[i]);
                i += columns;
            }
        }
        out
    }

    /// Inverts [`encode`] with the same `columns`.
    #[must_use]
    pub fn decode(data: &[u8], columns: NonZeroUsize) -> Vec<u8> {
        let columns = columns.get();
        let n = data.len();
        let mut out = vec![0u8; n];
        let mut pos = 0usize;
        for start in 0..columns {
            let mut i = start;
            while i < n {
                out[i] = data[pos];
                pos += 1;
                i += columns;
            }
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn nz(v: usize) -> NonZeroUsize {
            NonZeroUsize::new(v).unwrap()
        }

        #[test]
        fn roundtrip_empty() {
            let c = nz(1);
            assert_eq!(decode(&encode(&[], c), c), Vec::<u8>::new());
        }

        #[test]
        fn roundtrip_single_byte() {
            let c = nz(1);
            assert_eq!(decode(&encode(&[42], c), c), vec![42]);
        }

        #[test]
        fn roundtrip_fewer_rows_than_columns() {
            // columns wider than the data: every row has one element, so
            // both directions are the identity.
            let data = vec![1, 2, 3];
            let c = nz(8);
            assert_eq!(encode(&data, c), data);
            assert_eq!(decode(&data, c), data);
        }

        #[test]
        fn roundtrip_various_column_counts() {
            let data: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
            for columns in [1usize, 2, 3, 4, 8, 16, 96, 255, 1000, 1001] {
                let c = nz(columns);
                let encoded = encode(&data, c);
                assert_eq!(decode(&encoded, c), data, "columns {columns}");
            }
        }

        #[test]
        fn encode_groups_by_column() {
            // 2 columns, 3 rows (last row short): [a0 a1 | b0 b1 | c0] ->
            // column 0 then column 1.
            let data = vec![b'a', b'A', b'b', b'B', b'c'];
            let c = nz(2);
            assert_eq!(encode(&data, c), vec![b'a', b'b', b'c', b'A', b'B']);
        }

        #[test]
        fn single_column_is_identity() {
            let data = vec![10, 20, 30, 40, 50];
            let c = nz(1);
            assert_eq!(encode(&data, c), data);
            assert_eq!(decode(&data, c), data);
        }
    }
}
