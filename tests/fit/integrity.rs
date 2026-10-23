//! The integrity probe: every single-bit flip and every truncation of a
//! frame, none of which may decode to bytes other than the original.
//!
//! Two callers include this file with `#[path]`, so what counts as silent
//! has one definition: `tests/fit_lib.rs`'s `integrity-check` row, over its
//! fixed frames, and the `frame_probe` binary of `mothergod-bench`, over the
//! reviewer's input (issue #1210). The including file carries any
//! exclusion and any lint allowance for items it does not use.

/// What one probe covered and found.
#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    /// Length of the frame probed.
    pub bytes: usize,
    /// Single-bit flips decoded.
    pub flips: usize,
    /// Truncations decoded, each a proper prefix of the frame.
    pub truncations: usize,
    /// Mutations that decoded to bytes other than the original. Must be 0.
    pub silent: usize,
}

impl Report {
    /// Bit flips a frame of `bytes` offers: every bit of it.
    pub fn flip_space(&self) -> usize {
        self.bytes * 8
    }

    /// Truncations a frame of `bytes` offers: every proper prefix, the empty
    /// one included.
    pub fn truncation_space(&self) -> usize {
        self.bytes
    }
}

/// The `picks` positions of `0..total` a probe decodes: all of them when
/// `total <= cap`, else `cap` positions, one in each of `cap` equal cells of
/// `0..total`, so the sample covers the whole frame and no position repeats.
/// A cell's position is a fixed hash of its index: the same frame is probed
/// at the same bits every run, and a failure reruns.
pub fn positions(total: usize, cap: usize) -> impl Iterator<Item = usize> {
    let picks = total.min(cap);
    let wide = |n: usize| u64::try_from(n).expect("usize fits u64");
    (0..picks).map(move |cell| {
        let (cell, picks, total) = (wide(cell), wide(picks), wide(total));
        let (lo, hi) = (cell * total / picks, (cell + 1) * total / picks);
        // `picks <= total`, so a cell holds at least one position.
        let hash = cell.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32;
        usize::try_from(lo + hash % (hi - lo)).expect("below total, a usize")
    })
}

/// Probes `frame`, the compressed form of `payload`, through `decode`: at
/// most `cap` bit flips and `cap` truncations, exhaustive below `cap`.
/// `decode` is a parameter so a test can hand it a decoder that is wrong on
/// purpose and see `silent` count it.
pub fn probe_with<E>(
    payload: &[u8],
    frame: &[u8],
    decode: impl Fn(&[u8]) -> Result<Vec<u8>, E>,
    cap: usize,
) -> Report {
    let silent = |mutated: &[u8]| decode(mutated).is_ok_and(|out| out != payload);
    let mut report = Report {
        bytes: frame.len(),
        flips: 0,
        truncations: 0,
        silent: 0,
    };
    for bit in positions(report.flip_space(), cap) {
        let mut flipped = frame.to_vec();
        flipped[bit / 8] ^= 1 << (bit % 8);
        report.flips += 1;
        report.silent += usize::from(silent(&flipped));
    }
    for len in positions(report.truncation_space(), cap) {
        report.truncations += 1;
        report.silent += usize::from(silent(&frame[..len]));
    }
    report
}

/// [`probe_with`] over [`mothergod::compress`]'s frame of `payload` and
/// [`mothergod::decompress_bounded`], which must read the unmutated frame
/// back to `payload`: a probe of a codec that fails its own round trip
/// says nothing about its integrity.
///
/// # Panics
///
/// If `payload` is over `u32::MAX` bytes, or does not round-trip.
pub fn probe(payload: &[u8], cap: usize) -> Report {
    let frame = mothergod::compress(payload);
    let max_len = u32::try_from(payload.len().max(1 << 16)).expect("a payload below 4 GiB");
    let decode = |bytes: &[u8]| mothergod::decompress_bounded(bytes, max_len);
    assert_eq!(decode(&frame).as_deref(), Ok(payload), "round trip");
    probe_with(payload, &frame, decode, cap)
}
