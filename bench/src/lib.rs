//! Deterministic corpus generators for the mothergod benchmark harness.
//!
//! This is the train/sealed-validation tier described in
//! `research/corpus/POLICY.md`: seeded, in-repo, reproducible without
//! redistributing third-party data. The two mandatory dataset families
//! (`research/JOURNAL.md` S1-L3, corpus policy "Mandatory datasets") plus
//! the first structured class live here so far:
//!
//! - [`entropy_ladder`]: iid byte sources at a chosen order-0 entropy.
//! - [`markov_h8_2_trap`]: a uniform byte histogram with low (2 bit)
//!   conditional entropy, the histogram-coder trap.
//! - [`access_log`]: synthetic web-server access log lines, the
//!   "jsonl/log records" structured class.
//!
//! Ported by behavior (not code) from the founding session's Python
//! generator, `git show 1a3b1c8:research/imports/session-1/corpus.py`.
//! Silesia/Canterbury fetch-and-cache, the remaining structured classes,
//! and the sealed/train split plumbing are follow-up slices of
//! `research/JOURNAL.md` S1-D2.

use std::fmt::Write as _;

/// A small, fast, deterministic PRNG (`SplitMix64`). Not cryptographic; only
/// property this module needs is "same seed produces the same corpus".
struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "masked to the low byte, always fits u8"
    )]
    fn next_byte(&mut self) -> u8 {
        (self.next_u64() & 0xFF) as u8
    }

    /// Uniform integer in `[0, bound)`. Not bias-free for values of `bound`
    /// that don't divide `2^64`, but the residual skew is far below what a
    /// synthetic corpus generator needs to matter.
    fn next_range(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }

    /// Uniform index in `[0, bound)`, for indexing a small fixed-size slice.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "bound is always a small in-repo constant slice length, well within usize"
    )]
    fn next_index(&mut self, bound: usize) -> usize {
        self.next_range(bound as u64) as usize
    }

    /// Uniform float in `[0, 1)`, 53 bits of resolution.
    fn next_unit(&mut self) -> f64 {
        const TWO_POW_53: f64 = 9_007_199_254_740_992.0;
        // The shifted value has at most 53 significant bits, exactly
        // representable in an f64 mantissa (53 bits incl. the implicit one).
        #[allow(clippy::cast_precision_loss)]
        let mantissa = (self.next_u64() >> 11) as f64;
        mantissa / TWO_POW_53
    }
}

/// Geometric weights `[1, q, q^2, ..., q^255]`.
fn geometric_weights(q: f64) -> [f64; 256] {
    let mut weights = [0.0_f64; 256];
    let mut w = 1.0_f64;
    for slot in &mut weights {
        *slot = w;
        w *= q;
    }
    weights
}

/// Weights over the 256 byte values shaped geometrically, tuned by
/// bisection so the resulting distribution's Shannon entropy is
/// `target_h_bits`. `q` near 1 is flat (entropy 8); `q` near 0 concentrates
/// on byte 0 (entropy 0); entropy is monotone in `q`, so bisection converges.
fn skewed_weights(target_h_bits: f64) -> [f64; 256] {
    let entropy_for = |q: f64| -> f64 {
        let weights = geometric_weights(q);
        let total: f64 = weights.iter().sum();
        -weights
            .iter()
            .filter(|&&w| w > 0.0)
            .map(|&w| {
                let p = w / total;
                p * p.log2()
            })
            .sum::<f64>()
    };

    let (mut lo, mut hi) = (1e-6_f64, 0.999_999_f64);
    for _ in 0..60 {
        let mid = f64::midpoint(lo, hi);
        if entropy_for(mid) < target_h_bits {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    geometric_weights(f64::midpoint(lo, hi))
}

/// Draws one byte from `weights` (unnormalized) using `rng`.
fn sample_weighted(weights: &[f64; 256], rng: &mut Rng) -> u8 {
    let total: f64 = weights.iter().sum();
    let mut target = rng.next_unit() * total;
    let mut symbol: u8 = 0;
    for &weight in weights {
        if target < weight {
            return symbol;
        }
        target -= weight;
        symbol = symbol.wrapping_add(1);
    }
    255
}

/// Generates `len` iid bytes whose order-0 (histogram) Shannon entropy is
/// `target_h_bits` (one of 1, 2, 4, 6, 8 per the corpus policy ladder).
///
/// # Panics
///
/// Panics if `target_h_bits` is 0 or greater than 8; the ladder's own values
/// are always in range.
#[must_use]
pub fn entropy_ladder(target_h_bits: u8, len: usize, seed: u64) -> Vec<u8> {
    assert!(
        (1..=8).contains(&target_h_bits),
        "entropy_ladder target must be in 1..=8 bits, got {target_h_bits}"
    );
    let mut rng = Rng::new(seed);
    if target_h_bits == 8 {
        // Max entropy is exactly "uniform over all 256 values"; skip the
        // bisection, it would converge to q = 1 anyway.
        return (0..len).map(|_| rng.next_byte()).collect();
    }
    let weights = skewed_weights(f64::from(target_h_bits));
    (0..len)
        .map(|_| sample_weighted(&weights, &mut rng))
        .collect()
}

/// Generates `len` bytes with a uniform order-0 histogram (entropy ~8 bits)
/// but only 2 bits of conditional entropy given the previous byte: the
/// histogram-coder trap (`research/JOURNAL.md` S1-L3).
///
/// Construction: an additive random walk on `Z/256`. Each step's delta is
/// drawn iid from a distribution with entropy 2 bits; since addition mod 256
/// is a bijection for a fixed previous byte, `H(byte[i] | byte[i-1])
/// == H(delta) == 2` exactly, while the walk itself mixes to a uniform
/// marginal given enough steps.
#[must_use]
pub fn markov_h8_2_trap(len: usize, seed: u64) -> Vec<u8> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(len);
    if len == 0 {
        return out;
    }
    let weights = skewed_weights(2.0);
    let mut prev = rng.next_byte();
    out.push(prev);
    for _ in 1..len {
        let delta = sample_weighted(&weights, &mut rng);
        prev = prev.wrapping_add(delta);
        out.push(prev);
    }
    out
}

/// Source IP pool size for [`access_log`]: enough repetition that the same
/// address recurs across lines (real access logs are dominated by a small
/// set of clients), matching the founding generator's pool of 80.
const ACCESS_LOG_IP_POOL: usize = 80;

/// Request paths cycled by [`access_log`], mirroring the founding
/// generator's fixed small set.
const ACCESS_LOG_PATHS: [&str; 6] = [
    "/index.html",
    "/api/v2/users",
    "/api/v2/orders",
    "/static/app.js",
    "/favicon.ico",
    "/login",
];

/// Status codes cycled by [`access_log`], weighted toward 200 by repeating
/// it in the pool (three of five draws), matching the founding generator's
/// `random.choice([200, 200, 200, 304, 404])`.
const ACCESS_LOG_STATUSES: [u16; 5] = [200, 200, 200, 304, 404];

/// Generates `len` bytes of synthetic web-server access log lines (Apache
/// combined-log-style), the "jsonl/log records" structured class
/// (`research/corpus/POLICY.md`). Truncated to exactly `len` bytes; the
/// final line may be a partial one, matching how a real log tail is cut.
///
/// Ported by behavior (not code) from the founding session's `corpus.py`
/// (`c['log']`, `git show 1a3b1c8:research/imports/session-1/corpus.py`):
/// a pool of source IPs, a fixed small set of request paths, an
/// incrementing per-line timestamp, and a status code skewed toward 200.
#[must_use]
pub fn access_log(len: usize, seed: u64) -> Vec<u8> {
    if len == 0 {
        return Vec::new();
    }
    let mut rng = Rng::new(seed);
    let ips: Vec<String> = (0..ACCESS_LOG_IP_POOL)
        .map(|_| {
            format!(
                "{}.{}.{}.{}",
                1 + rng.next_range(254),
                rng.next_range(255),
                rng.next_range(255),
                rng.next_range(255)
            )
        })
        .collect();

    let mut out = String::with_capacity(len);
    let mut line = 0usize;
    while out.len() < len {
        let ip = &ips[rng.next_index(ACCESS_LOG_IP_POOL)];
        let path = ACCESS_LOG_PATHS[rng.next_index(ACCESS_LOG_PATHS.len())];
        let status = ACCESS_LOG_STATUSES[rng.next_index(ACCESS_LOG_STATUSES.len())];
        let size = 200 + rng.next_range(49_800);
        let minute = (line / 60) % 60;
        let second = line % 60;
        writeln!(
            out,
            "{ip} - - [19/Aug/2026:10:{minute:02}:{second:02} +0200] \
             \"GET {path} HTTP/1.1\" {status} {size}"
        )
        .expect("writing to a String never fails");
        line += 1;
    }
    out.truncate(len);
    out.into_bytes()
}

/// Order-0 (histogram) Shannon entropy of `data`, in bits/byte. `0.0` for
/// empty input.
///
/// Counts and length stay far below 2^53 for any corpus this crate
/// generates, so the `f64` conversions below are exact.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn order0_entropy_bits(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u64; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let len = data.len() as f64;
    -counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / len;
            p * p.log2()
        })
        .sum::<f64>()
}

/// Order-1 conditional entropy `H(byte[i] | byte[i-1])`, in bits/byte,
/// estimated from `data`. `0.0` for inputs shorter than 2 bytes.
///
/// Counts and length stay far below 2^53 for any corpus this crate
/// generates, so the `f64` conversions below are exact.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn order1_conditional_entropy_bits(data: &[u8]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let mut joint = vec![0u64; 256 * 256];
    let mut marginal = [0u64; 256];
    for window in data.windows(2) {
        let (prev, cur) = (window[0] as usize, window[1] as usize);
        joint[prev * 256 + cur] += 1;
        marginal[prev] += 1;
    }
    let total = (data.len() - 1) as f64;
    -joint
        .iter()
        .enumerate()
        .filter(|&(_, &c)| c > 0)
        .map(|(idx, &c)| {
            let prev = idx / 256;
            let p_joint = c as f64 / total;
            let p_cond = c as f64 / marginal[prev] as f64;
            p_joint * p_cond.log2()
        })
        .sum::<f64>()
}

#[cfg(test)]
mod tests {
    use super::{
        access_log, entropy_ladder, markov_h8_2_trap, order0_entropy_bits,
        order1_conditional_entropy_bits,
    };

    const LEN: usize = 200_000;
    const SEED: u64 = 0xC0FF_EE12_3456_789A;

    #[test]
    fn entropy_ladder_hits_target_entropy() {
        for target in [1u8, 2, 4, 6, 8] {
            let data = entropy_ladder(target, LEN, SEED);
            let measured = order0_entropy_bits(&data);
            assert!(
                (measured - f64::from(target)).abs() < 0.05,
                "target {target} bits, measured {measured:.3} bits"
            );
        }
    }

    #[test]
    fn entropy_ladder_is_deterministic() {
        assert_eq!(entropy_ladder(4, 1000, SEED), entropy_ladder(4, 1000, SEED));
    }

    #[test]
    fn entropy_ladder_seeds_are_independent() {
        assert_ne!(
            entropy_ladder(4, 1000, SEED),
            entropy_ladder(4, 1000, SEED + 1)
        );
    }

    #[test]
    fn markov_trap_has_near_uniform_marginal_histogram() {
        let data = markov_h8_2_trap(LEN, SEED);
        let measured = order0_entropy_bits(&data);
        // Max possible is 8.0; a fully collapsed histogram would read near 0.
        // The random walk needs to mix, so allow more slack than the ladder.
        assert!(
            measured > 7.5,
            "expected a near-uniform marginal histogram, measured {measured:.3} bits"
        );
    }

    #[test]
    fn markov_trap_has_low_conditional_entropy() {
        let data = markov_h8_2_trap(LEN, SEED);
        let measured = order1_conditional_entropy_bits(&data);
        assert!(
            (measured - 2.0).abs() < 0.05,
            "expected ~2 bits conditional entropy, measured {measured:.3} bits"
        );
    }

    #[test]
    fn markov_trap_separates_histogram_from_context_models() {
        let data = markov_h8_2_trap(LEN, SEED);
        let h0 = order0_entropy_bits(&data);
        let h1 = order1_conditional_entropy_bits(&data);
        // The whole point of this dataset (JOURNAL S1-L3): a histogram coder
        // sees ~8 bits/byte, a context model sees ~2.
        assert!(
            h0 - h1 > 5.0,
            "h0={h0:.3} h1={h1:.3}, gap too small to trap a histogram coder"
        );
    }

    #[test]
    // Both functions return the literal 0.0 on the empty-input fast path,
    // not a computed float, so exact comparison is the right check.
    #[allow(clippy::float_cmp)]
    fn empty_inputs_are_handled() {
        assert_eq!(entropy_ladder(4, 0, SEED), Vec::<u8>::new());
        assert_eq!(markov_h8_2_trap(0, SEED), Vec::<u8>::new());
        assert_eq!(access_log(0, SEED), Vec::<u8>::new());
        assert_eq!(order0_entropy_bits(&[]), 0.0);
        assert_eq!(order1_conditional_entropy_bits(&[]), 0.0);
    }

    #[test]
    fn access_log_is_exactly_the_requested_length() {
        for len in [1, 2, 47, 1000, LEN] {
            assert_eq!(access_log(len, SEED).len(), len);
        }
    }

    #[test]
    fn access_log_is_deterministic() {
        assert_eq!(access_log(5_000, SEED), access_log(5_000, SEED));
    }

    #[test]
    fn access_log_seeds_are_independent() {
        assert_ne!(access_log(5_000, SEED), access_log(5_000, SEED + 1));
    }

    #[test]
    fn access_log_looks_like_log_lines() {
        let data = access_log(LEN, SEED);
        let text = String::from_utf8(data).expect("generator only emits ASCII");
        let full_lines: Vec<&str> = text.lines().filter(|l| l.len() > 40).collect();
        assert!(full_lines.len() > 100, "expected many full log lines");
        for line in &full_lines {
            assert!(line.contains("GET"), "line missing request verb: {line}");
            assert!(line.contains("HTTP/1.1"), "line missing protocol: {line}");
        }
    }

    #[test]
    fn access_log_repeats_a_small_ip_pool() {
        // Real access logs are dominated by a handful of clients; a large
        // corpus generated from a fixed 80-address pool must show far
        // fewer distinct leading octets than lines, unlike iid random data.
        let text = String::from_utf8(access_log(LEN, SEED)).expect("ASCII only");
        let distinct_ips: std::collections::HashSet<&str> = text
            .lines()
            .filter_map(|l| l.split(' ').next())
            .filter(|ip| !ip.is_empty())
            .collect();
        assert!(
            distinct_ips.len() <= 80,
            "expected at most the 80-address pool, got {}",
            distinct_ips.len()
        );
    }

    #[test]
    fn generators_round_trip_through_the_frame_format() {
        for data in [
            entropy_ladder(1, 5_000, SEED),
            entropy_ladder(8, 5_000, SEED),
            markov_h8_2_trap(5_000, SEED),
            access_log(5_000, SEED),
        ] {
            assert_eq!(mothergod::decompress(&mothergod::compress(&data)), Ok(data));
        }
    }
}
