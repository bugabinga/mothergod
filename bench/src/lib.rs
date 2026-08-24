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
//! - [`json_records`]: a synthetic JSON API response, the "json"
//!   structured class.
//! - [`base64_wrapped`]: a base64-wrapped text payload, the
//!   "base64-wrapped payloads" structured class.
//! - [`interleaved_audio16`]: interleaved 16-bit audio samples, the
//!   "audio" structured class.
//! - [`gradient_image`]: a synthetic grayscale gradient image, the
//!   "gradient image" structured class.
//! - [`sqlite_like_records`]: fixed-width binary rows over a
//!   timestamp/category/measurement schema, the "sqlite-like records"
//!   structured class.
//! - [`x86_dense_code`]: a synthetic instruction stream dense with
//!   `call`/`jmp rel32` opcodes, the "x86-dense binaries" structured class.
//!
//! Ported by behavior (not code) from the founding session's Python
//! generator, `git show 1a3b1c8:research/imports/session-1/corpus.py`.
//!
//! The `corpus` module (behind the opt-in `corpus-fetch` feature, so it
//! isn't in scope for this doc build's default features) fetches and
//! caches the held-out-final corpora (Silesia, Canterbury) pinned in
//! `bench/corpus.toml`. [`train_window`] is the first slice of the
//! train/sealed split plumbing (`research/JOURNAL.md` S1-D2): a rotating
//! window over a generator's output, so repeated experiment iterations
//! see a different offset instead of memorizing one. [`sealed_seed`]
//! is the second slice: deriving a sealed-validation seed, distinct from
//! its train seed, for the same generator. [`DatasetKind`] is the third:
//! which generator kinds are sealed-only, never appearing in train.
//! [`regret`] scores a candidate corpus addition once those three exist to
//! feed it real numbers.

#[cfg(feature = "corpus-fetch")]
pub mod corpus;

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

/// Probability [`json_records`] sets a record's `active` field to `true`,
/// matching the founding generator's `random.random() < 0.8`.
const JSON_ACTIVE_PROBABILITY: f64 = 0.8;

/// Draws one standard-normal (mean 0, stddev 1) sample via the Box-Muller
/// transform. `rng.next_unit()` returns `[0, 1)`; shifting one draw to `(0,
/// 1]` keeps the log finite.
fn standard_normal(rng: &mut Rng) -> f64 {
    let u1 = 1.0 - rng.next_unit();
    let u2 = rng.next_unit();
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

/// Generates `len` bytes of a synthetic JSON API response, the "json"
/// structured class (`research/corpus/POLICY.md`): a `{"status": "ok",
/// "results": [...]}` envelope around user records (`user_id`, `name`,
/// `email`, `active`, `score`). Truncated to exactly `len` bytes; mid-record
/// truncation is not repaired, matching how the archive's own
/// `json.dumps(resp).encode()[:N]` truncation works (the result is not
/// guaranteed to be valid JSON when `len` cuts inside the document).
///
/// Ported by behavior (not code) from the founding session's `corpus.py`
/// (`c['json']`, `git show 1a3b1c8:research/imports/session-1/corpus.py`):
/// records with a gaussian `score` (mean 50, stddev 15) and `active` true
/// 80% of the time. One behavior-preserving deviation, matching
/// [`access_log`]'s: the archive fixes the response at 500 records then
/// truncates to `N` bytes; this generates records until `len` bytes are
/// reached, so it produces exactly `len` bytes for any requested length
/// instead of only for the one size the archive's fixed record count
/// happened to cover.
#[must_use]
pub fn json_records(len: usize, seed: u64) -> Vec<u8> {
    if len == 0 {
        return Vec::new();
    }
    let mut rng = Rng::new(seed);
    let mut out = String::with_capacity(len);
    out.push_str(r#"{"status": "ok", "results": ["#);
    let mut i = 0usize;
    while out.len() < len {
        if i > 0 {
            out.push_str(", ");
        }
        let user_id = 1000 + i;
        let active = rng.next_unit() < JSON_ACTIVE_PROBABILITY;
        let score = 50.0 + 15.0 * standard_normal(&mut rng);
        write!(
            out,
            r#"{{"user_id": {user_id}, "name": "user_{i}", "email": "user_{i}@example.com", "active": {active}, "score": {score:.1}}}"#
        )
        .expect("writing to a String never fails");
        i += 1;
    }
    out.push_str("]}");
    out.truncate(len);
    out.into_bytes()
}

/// Standard base64 alphabet (RFC 4648, `+`/`/`, `=` padding). A second,
/// from-scratch copy of the table `src/filters.rs`'s `base64_unwrap` filter
/// also carries: that one is private to a codec transform's own round trip,
/// this crate never reaches into `src/` internals for corpus generation
/// (every generator here, `entropy_ladder` through `json_records`, is
/// self-contained), and RFC 4648's alphabet is a fixed public standard, not
/// project logic, so the duplication carries no drift risk.
const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes `data` as standard base64 with `=` padding (RFC 4648).
fn base64_encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied();
        let b2 = chunk.get(2).copied();
        out.push(BASE64_ALPHABET[(b0 >> 2) as usize]);
        out.push(BASE64_ALPHABET[(((b0 & 0x03) << 4) | (b1.unwrap_or(0) >> 4)) as usize]);
        out.push(match b1 {
            Some(b1) => BASE64_ALPHABET[(((b1 & 0x0F) << 2) | (b2.unwrap_or(0) >> 6)) as usize],
            None => b'=',
        });
        out.push(match b2 {
            Some(b2) => BASE64_ALPHABET[(b2 & 0x3F) as usize],
            None => b'=',
        });
    }
    out
}

/// Generates `len` bytes of a base64-wrapped text payload, the
/// "base64-wrapped payloads" structured class (`research/corpus/POLICY.md`).
/// Truncated to exactly `len` bytes.
///
/// Ported by behavior (not code) from the founding session's `corpus.py`
/// (`c['b64-text']`, `git show 1a3b1c8:research/imports/session-1/corpus.py`):
/// base64-encode a text-like payload and truncate to length. The archive
/// draws its text from `/usr/share/doc/*/copyright` on the host
/// filesystem, neither deterministic nor available in every environment;
/// this port substitutes [`json_records`], this module's own synthetic
/// text source, keeping the same "compressible source pushed through
/// base64's 6-bit encoding" shape. The archive's second variant,
/// `b64-random` (base64 of `os.urandom`), is not ported: `entropy_ladder`
/// already covers a maximum-entropy source, and wrapping it in base64
/// changes only the alphabet, not the coverage.
#[must_use]
pub fn base64_wrapped(len: usize, seed: u64) -> Vec<u8> {
    if len == 0 {
        return Vec::new();
    }
    // base64 expands n input bytes to 4*ceil(n/3) output bytes, which is
    // >= n for every n >= 1, so requesting `len` underlying bytes always
    // yields at least `len` encoded bytes; the excess is truncated below.
    let underlying = json_records(len, seed);
    let mut encoded = base64_encode(&underlying);
    encoded.truncate(len);
    encoded
}

/// Amplitude of [`interleaved_audio16`]'s slower sine component (period 37
/// samples), matching the founding generator's `2500*sin(i/37)`.
const AUDIO_AMP_SLOW: f64 = 2500.0;

/// Period divisor of [`interleaved_audio16`]'s slower sine component.
const AUDIO_PERIOD_SLOW: f64 = 37.0;

/// Amplitude of [`interleaved_audio16`]'s faster sine component (period 11
/// samples), matching the founding generator's `1500*sin(i/11)`.
const AUDIO_AMP_FAST: f64 = 1500.0;

/// Period divisor of [`interleaved_audio16`]'s faster sine component.
const AUDIO_PERIOD_FAST: f64 = 11.0;

/// Standard deviation of [`interleaved_audio16`]'s additive gaussian noise,
/// matching the founding generator's `200*random.gauss(0, 1)`.
const AUDIO_NOISE_STDDEV: f64 = 200.0;

/// Generates `len` bytes of interleaved 16-bit audio samples (little-endian),
/// the "audio" structured class (`research/corpus/POLICY.md`). Truncated to
/// exactly `len` bytes; an odd `len` ends mid-sample, keeping only that
/// sample's low byte.
///
/// Ported by behavior (not code) from the founding session's `corpus.py`
/// (`c['audio16']`, `git show 1a3b1c8:research/imports/session-1/corpus.py`):
/// each sample sums a slow sine (amplitude 2500, period 37 samples), a fast
/// sine (amplitude 1500, period 11 samples), and gaussian noise (stddev
/// 200), then truncates toward zero and keeps the low 16 bits. Python's
/// `int(...)` truncates a float toward zero exactly like `as i64` does, and
/// its `& 0xffff` on a (possibly negative) arbitrary-precision int keeps the
/// low 16 bits in two's complement, exactly what `as u16` produces from that
/// `i64`. One behavior-preserving deviation, the same shape as
/// [`access_log`]'s: the archive fixes the sample count at `N / 2` then
/// emits exactly that many bytes (so an odd `N` loses its last requested
/// byte entirely); this generates samples until `len` bytes are reached then
/// truncates, so it produces exactly `len` bytes for any requested length.
#[must_use]
pub fn interleaved_audio16(len: usize, seed: u64) -> Vec<u8> {
    if len == 0 {
        return Vec::new();
    }
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(len + 1);
    let mut sample_index = 0u64;
    while out.len() < len {
        // Sample indices stay far below 2^53 for any corpus this crate
        // generates, so this conversion is exact.
        #[allow(clippy::cast_precision_loss)]
        let phase = sample_index as f64;
        let noise = AUDIO_NOISE_STDDEV * standard_normal(&mut rng);
        let sample = AUDIO_AMP_SLOW * (phase / AUDIO_PERIOD_SLOW).sin()
            + AUDIO_AMP_FAST * (phase / AUDIO_PERIOD_FAST).sin()
            + noise;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "truncating toward zero, matching Python's int(), before the intentional u16 wraparound below"
        )]
        let truncated = sample.trunc() as i64;
        #[allow(
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            reason = "wraps to the low 16 bits in two's complement, matching Python's `& 0xffff` on a signed int"
        )]
        let v = truncated as u16;
        out.push((v & 0xff) as u8);
        if out.len() < len {
            out.push((v >> 8) as u8);
        }
        sample_index += 1;
    }
    out
}

/// Row width of [`gradient_image`]'s synthetic grayscale image (200 pixels
/// per row), matching the founding generator's fixed row length.
const IMAGE_WIDTH: usize = 200;

/// Baseline pixel value of [`gradient_image`], matching the founding
/// generator's `90 + ...`.
const IMAGE_BASELINE: f64 = 90.0;

/// Amplitude of [`gradient_image`]'s horizontal sine component (period 31
/// pixels along a row), matching the founding generator's `70*sin(x/31)`.
const IMAGE_AMP_X: f64 = 70.0;

/// Period divisor of [`gradient_image`]'s horizontal sine component.
const IMAGE_PERIOD_X: f64 = 31.0;

/// Amplitude of [`gradient_image`]'s vertical sine component (period 23
/// rows), matching the founding generator's `50*sin(y/23)`.
const IMAGE_AMP_Y: f64 = 50.0;

/// Period divisor of [`gradient_image`]'s vertical sine component.
const IMAGE_PERIOD_Y: f64 = 23.0;

/// Standard deviation of [`gradient_image`]'s additive gaussian noise,
/// matching the founding generator's `8*random.gauss(0, 1)`.
const IMAGE_NOISE_STDDEV: f64 = 8.0;

/// Generates `len` bytes of a synthetic grayscale gradient image, the
/// "gradient image" structured class (`research/corpus/POLICY.md`).
/// Truncated to exactly `len` bytes; an odd `len` ends mid-row, keeping a
/// partial final row, matching how a real image scan would be cut.
///
/// Ported by behavior (not code) from the founding session's `corpus.py`
/// (`c['image']`, `git show 1a3b1c8:research/imports/session-1/corpus.py`):
/// pixels in row-major order over 200-pixel-wide rows, each the sum of a
/// baseline (90), a horizontal sine (amplitude 70, period 31 pixels), a
/// vertical sine (amplitude 50, period 23 rows), and gaussian noise (stddev
/// 8), truncated toward zero and kept to the low byte. Python's `int(...)`
/// truncates toward zero exactly like `as i32` does, and its `& 0xff` on a
/// (possibly negative) arbitrary-precision int keeps the low byte in two's
/// complement, exactly what `as u8` produces from that `i32`. One
/// behavior-preserving deviation, the same shape as [`interleaved_audio16`]'s:
/// the archive fixes the row count at `N / 200 + 1` then truncates the
/// flattened result to `N` bytes; this generates pixels until `len` bytes
/// are reached then stops, so it produces exactly `len` bytes for any
/// requested length instead of only for sizes the archive's fixed row count
/// happened to cover.
#[must_use]
pub fn gradient_image(len: usize, seed: u64) -> Vec<u8> {
    if len == 0 {
        return Vec::new();
    }
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(len);
    let mut x = 0usize;
    let mut y = 0usize;
    while out.len() < len {
        // Row/column indices stay far below 2^53 for any corpus this crate
        // generates, so these conversions are exact.
        #[allow(clippy::cast_precision_loss)]
        let (xf, yf) = (x as f64, y as f64);
        let noise = IMAGE_NOISE_STDDEV * standard_normal(&mut rng);
        let value = IMAGE_BASELINE
            + IMAGE_AMP_X * (xf / IMAGE_PERIOD_X).sin()
            + IMAGE_AMP_Y * (yf / IMAGE_PERIOD_Y).sin()
            + noise;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "truncating toward zero, matching Python's int(), before the intentional u8 wraparound below"
        )]
        let truncated = value.trunc() as i32;
        #[allow(
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            reason = "wraps to the low byte in two's complement, matching Python's `& 0xff` on a signed int"
        )]
        let pixel = truncated as u8;
        out.push(pixel);
        x += 1;
        if x == IMAGE_WIDTH {
            x = 0;
            y += 1;
        }
    }
    out
}

/// Starting timestamp (Unix seconds) of [`sqlite_like_records`], matching
/// the founding generator's `1700000000 + i*60`.
const SQLITE_TS_START: i64 = 1_700_000_000;

/// Per-row timestamp increment (seconds) of [`sqlite_like_records`],
/// matching the founding generator's `i*60`.
const SQLITE_TS_STEP: i64 = 60;

/// Fixed-width (null-padded) category values [`sqlite_like_records`] cycles
/// through, matching the founding generator's
/// `random.choice(['temp', 'hum', 'pres'])`. Each entry is exactly
/// [`SQLITE_CATEGORY_WIDTH`] bytes so every row is the same byte width.
const SQLITE_CATEGORIES: [[u8; SQLITE_CATEGORY_WIDTH]; 3] = [*b"temp", *b"hum\0", *b"pres"];

/// Byte width of [`sqlite_like_records`]'s category field.
const SQLITE_CATEGORY_WIDTH: usize = 4;

/// Mean of [`sqlite_like_records`]'s measurement field, matching the
/// founding generator's `random.gauss(20, 3)`.
const SQLITE_VALUE_MEAN: f64 = 20.0;

/// Standard deviation of [`sqlite_like_records`]'s measurement field,
/// matching the founding generator's `random.gauss(20, 3)`.
const SQLITE_VALUE_STDDEV: f64 = 3.0;

/// Byte width of one [`sqlite_like_records`] row: an 8-byte little-endian
/// timestamp, a 4-byte category, and an 8-byte little-endian measurement.
const SQLITE_ROW_WIDTH: usize = 8 + SQLITE_CATEGORY_WIDTH + 8;

/// Generates `len` bytes of fixed-width binary rows over a
/// timestamp/category/measurement schema, the "sqlite-like records"
/// structured class (`research/corpus/POLICY.md`). Truncated to exactly
/// `len` bytes; a `len` not a multiple of the row width ends mid-row.
///
/// Ported by behavior (not code) from the founding session's `corpus.py`
/// (`c['sqlite']`, `git show 1a3b1c8:research/imports/session-1/corpus.py`):
/// the archive opened a real `sqlite3` connection, created `table
/// m(ts int, s text, v real)`, inserted rows of a linearly increasing
/// timestamp (`1700000000 + i*60`), a category drawn from `{temp, hum,
/// pres}`, and a gaussian measurement (mean 20, stddev 3), then read the
/// resulting database file's raw bytes. That file's exact byte layout
/// (page size, freelist state, B-tree structure, per-value varint serial
/// types) was never a design choice in the archive, only whatever the
/// installed `sqlite3` library happened to emit, and reproducing it exactly
/// would mean re-implementing `SQLite`'s on-disk format — out of scope for a
/// zero-dependency corpus generator (ADR-0002) and not what "sqlite-like"
/// asks for. This port instead captures the schema's shape directly: fixed-
/// width rows (8-byte timestamp, 4-byte null-padded category, 8-byte
/// measurement, all little-endian), which exercises the same repeated-
/// structure/mixed-type compression opportunity the class exists to probe.
#[must_use]
pub fn sqlite_like_records(len: usize, seed: u64) -> Vec<u8> {
    if len == 0 {
        return Vec::new();
    }
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(len + SQLITE_ROW_WIDTH);
    let mut i: i64 = 0;
    while out.len() < len {
        let ts = SQLITE_TS_START + i * SQLITE_TS_STEP;
        let category = SQLITE_CATEGORIES[rng.next_index(SQLITE_CATEGORIES.len())];
        let value = SQLITE_VALUE_MEAN + SQLITE_VALUE_STDDEV * standard_normal(&mut rng);
        out.extend_from_slice(&ts.to_le_bytes());
        out.extend_from_slice(&category);
        out.extend_from_slice(&value.to_le_bytes());
        i += 1;
    }
    out.truncate(len);
    out
}

/// Number of distinct call/jmp targets [`x86_dense_code`] draws from,
/// simulating a binary whose calls cluster on a handful of functions
/// (`src/filters.rs`'s `bcj` doc comment: "many calls target the same
/// handful of functions").
const X86_FUNCTION_COUNT: usize = 48;

/// Byte spacing between consecutive synthetic function starts in
/// [`x86_dense_code`]'s target pool, matching typical compiled-function
/// density closely enough to give `call`/`jmp` targets a realistic spread.
const X86_FUNCTION_STRIDE: i64 = 64;

/// Byte length of a `call rel32`/`jmp rel32` instruction: one opcode byte
/// plus a 4-byte little-endian operand, matching `src/filters.rs`'s `bcj`
/// filter's `INSTRUCTION_LEN`.
const X86_CALL_LEN: usize = 5;

/// Chance each emitted instruction is a `call`/`jmp rel32` rather than a
/// filler instruction, tuned high on purpose: "dense" is the point of this
/// class (`research/corpus/POLICY.md`'s "x86-dense binaries"), stressing
/// the `bcj` filter (S2-A4) far harder than typical compiled code would.
const X86_CALL_PROBABILITY: f64 = 0.25;

/// A small pool of short, common x86-64 instruction encodings
/// [`x86_dense_code`] cycles through as filler between `call`/`jmp`
/// instructions: prologue/epilogue bytes, register moves, arithmetic,
/// comparisons and short conditional jumps. Chosen for shape (short,
/// repeated opcodes), not to encode any particular program.
const X86_FILLER_INSTRUCTIONS: [&[u8]; 15] = [
    &[0x55],                   // push rbp
    &[0x5D],                   // pop rbp
    &[0xC3],                   // ret
    &[0x90],                   // nop
    &[0x48, 0x89, 0xE5],       // mov rbp, rsp
    &[0x48, 0x83, 0xEC, 0x18], // sub rsp, 0x18
    &[0x48, 0x83, 0xC4, 0x18], // add rsp, 0x18
    &[0x85, 0xC0],             // test eax, eax
    &[0x31, 0xC0],             // xor eax, eax
    &[0x89, 0xF8],             // mov eax, edi
    &[0x39, 0xD8],             // cmp eax, ebx
    &[0x74, 0x05],             // je +5
    &[0x75, 0x05],             // jne +5
    &[0x48, 0x8B, 0x45, 0xF8], // mov rax, [rbp-8]
    &[0x48, 0x89, 0x45, 0xF8], // mov [rbp-8], rax
];

/// Generates `len` bytes of a synthetic x86-64 instruction stream dense
/// with `call`/`jmp rel32` opcodes, the "x86-dense binaries" structured
/// class (`research/corpus/POLICY.md`). Truncated to exactly `len` bytes;
/// a `call`/`jmp` instruction split by truncation loses its trailing
/// operand bytes, matching how a real binary tail is cut.
///
/// Ported by behavior (not code) from the founding session's `corpus.py`
/// (`c['elf']`, `git show 1a3b1c8:research/imports/session-1/corpus.py`):
/// the archive reads a slice of the host's installed `libc.so.6`, neither
/// deterministic nor available in every environment (varies by libc
/// version, and there is nothing to read on a host without one). This
/// port substitutes a synthetic instruction stream, the same deviation
/// shape as [`sqlite_like_records`]'s: rather than real machine code, it
/// captures the structural property the class exists to probe (`bcj`'s
/// doc comment on `src/filters.rs`) — `call`/`jmp rel32` operands that
/// cluster in absolute-address space because calls target a small pool of
/// functions repeatedly. Instructions are drawn from a small filler pool
/// (prologue/epilogue, register moves, arithmetic, short conditional
/// jumps) with a 25% chance of emitting a `call`/`jmp rel32` to one of 48
/// synthetic function starts instead.
#[must_use]
pub fn x86_dense_code(len: usize, seed: u64) -> Vec<u8> {
    if len == 0 {
        return Vec::new();
    }
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(len + X86_CALL_LEN);
    while out.len() < len {
        if rng.next_unit() < X86_CALL_PROBABILITY {
            let opcode = if rng.next_unit() < 0.5 { 0xE8 } else { 0xE9 };
            #[allow(
                clippy::cast_possible_wrap,
                reason = "X86_FUNCTION_COUNT is a small in-repo constant; the product stays far below i64::MAX"
            )]
            let target = rng.next_index(X86_FUNCTION_COUNT) as i64 * X86_FUNCTION_STRIDE;
            // rel32 is measured from the address of the following
            // instruction, i.e. this opcode's position plus the 5-byte
            // instruction length; wraps like real rel32 addressing if the
            // synthetic target pool were ever large enough to overflow it,
            // which it never is here.
            #[allow(
                clippy::cast_possible_wrap,
                reason = "out.len() stays far below i64::MAX for any corpus this crate generates"
            )]
            let pos_after = out.len() as i64 + X86_CALL_LEN as i64;
            #[allow(
                clippy::cast_possible_truncation,
                reason = "target and pos_after both stay within a few thousand bytes of each other, well within i32 range"
            )]
            let rel32 = (target - pos_after) as i32;
            out.push(opcode);
            out.extend_from_slice(&rel32.to_le_bytes());
        } else {
            let instr = X86_FILLER_INSTRUCTIONS[rng.next_index(X86_FILLER_INSTRUCTIONS.len())];
            out.extend_from_slice(instr);
        }
    }
    out.truncate(len);
    out
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

/// Returns a rotating window of `data`, `window_len` bytes long, whose
/// start offset changes with `iteration` (`research/corpus/POLICY.md`,
/// "Train slices — rotating windows over each dataset; a different
/// window every iteration so offsets can't be memorized"). The window
/// wraps circularly around `data`, so every `iteration` yields exactly
/// `window_len` bytes and the sequence of windows cycles back to the
/// start once `iteration` has advanced past `data.len()`.
///
/// # Panics
///
/// Panics if `window_len` is `0` or exceeds `data.len()`: there is no
/// well-defined window to take.
#[must_use]
pub fn train_window(data: &[u8], window_len: usize, iteration: u64) -> Vec<u8> {
    assert!(window_len > 0, "train_window: window_len must be nonzero");
    assert!(
        window_len <= data.len(),
        "train_window: window_len exceeds data.len()"
    );

    let len = data.len() as u64;
    let start = usize::try_from(iteration % len).expect("modulo data.len() fits in usize");
    let end = start + window_len;
    if end <= data.len() {
        data[start..end].to_vec()
    } else {
        let mut window = Vec::with_capacity(window_len);
        window.extend_from_slice(&data[start..]);
        window.extend_from_slice(&data[..end - data.len()]);
        window
    }
}

/// Fixed key mixed into a train seed before deriving its sealed-validation
/// counterpart, distinguishing "seed run through [`sealed_seed`]" from "seed
/// picked directly for train".
const SEALED_SEED_KEY: u64 = 0x5EA1_ED5E_A1ED_5EA1;

/// Derives a sealed-validation seed from a train seed
/// (`research/corpus/POLICY.md`, "Sealed validation set — different seed...
/// from train"). Feeds `train_seed` (`XOR`ed with a fixed key) through the
/// same `SplitMix64` step `Rng` uses. That step is a bijection on `u64`
/// (the `wrapping_add` and the xorshift-multiply avalanche are each
/// invertible), so distinct train seeds always derive distinct sealed
/// seeds and every sealed seed traces back to exactly one train seed. It
/// does not prove a sealed seed can never coincide with some unrelated
/// seed chosen directly for train — that would need a structural split
/// (e.g. a reserved bit), which conflicts with seeds already in use
/// elsewhere in this crate that set every bit pattern (S2-A1's
/// `0xC0FF_EE12_3456_789A`). Same caveat as `Rng`: reproducible and distinct
/// from its own input, not cryptographically unpredictable.
///
/// This is the seed half of "held-out seeds AND held-out dataset kinds"
/// (POLICY.md). [`DatasetKind::sealed_only`] is the dataset-kind half.
#[must_use]
pub fn sealed_seed(train_seed: u64) -> u64 {
    Rng::new(train_seed ^ SEALED_SEED_KEY).next_u64()
}

/// One of this crate's deterministic corpus generators, named so the
/// train/sealed dataset-kind split (`research/corpus/POLICY.md`: "held-out
/// seeds AND held-out dataset kinds") can designate some kinds sealed-only
/// without every caller re-deriving that list from the generator functions
/// themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatasetKind {
    /// [`entropy_ladder`]: the order-0 entropy floor check.
    EntropyLadder,
    /// [`markov_h8_2_trap`]: the histogram-coder trap.
    MarkovH82Trap,
    /// [`access_log`]: the jsonl/log-records structured class.
    AccessLog,
    /// [`json_records`]: the json structured class.
    JsonRecords,
    /// [`base64_wrapped`]: the base64-wrapped-payload structured class.
    Base64Wrapped,
    /// [`interleaved_audio16`]: the audio structured class.
    InterleavedAudio16,
    /// [`gradient_image`]: the gradient-image structured class.
    GradientImage,
    /// [`sqlite_like_records`]: the sqlite-like-records structured class.
    SqliteLikeRecords,
    /// [`x86_dense_code`]: the x86-dense-binaries structured class.
    X86DenseCode,
}

impl DatasetKind {
    /// Every generator kind, in the order this module defines them.
    pub const ALL: [Self; 9] = [
        Self::EntropyLadder,
        Self::MarkovH82Trap,
        Self::AccessLog,
        Self::JsonRecords,
        Self::Base64Wrapped,
        Self::InterleavedAudio16,
        Self::GradientImage,
        Self::SqliteLikeRecords,
        Self::X86DenseCode,
    ];

    /// Whether this kind is sealed-validation-only: train-slice code must
    /// never request it (`research/corpus/POLICY.md`, "held-out dataset
    /// kinds"; no agent ever tunes against the sealed set).
    ///
    /// [`Self::EntropyLadder`] and [`Self::MarkovH82Trap`] are POLICY's
    /// mandatory datasets — they check the coder against the theoretical
    /// floor and the histogram-coder trap on every train iteration, not
    /// generalization, so both stay in train. Of the remaining five,
    /// [`Self::InterleavedAudio16`] (delta), [`Self::X86DenseCode`] (BCJ),
    /// [`Self::Base64Wrapped`] (base64-unwrap), and
    /// [`Self::SqliteLikeRecords`] (transpose — its own doc comment names
    /// "a fixed record width" as the target shape, and at this
    /// generator's true 20-byte row width transpose measurably lowers
    /// order-1 entropy by 0.94 bits/byte, verified by running the actual
    /// filter, not assumed from the doc) each have a filter in
    /// `src/filters.rs` whose documented purpose matches their shape, so
    /// train slices actively exercise that filter against them.
    /// [`Self::AccessLog`] and [`Self::GradientImage`] are held
    /// sealed-only instead: no filter's documented purpose matches
    /// either shape, and scanning every delta stride (1..=96) and
    /// transpose column count (2..=96) against each finds nothing
    /// resembling the sqlite case — the best incidental win either shows
    /// stays under 0.15 bits/byte, noise next to the four filter-covered
    /// kinds' effects. These two measure whether the parse/model/coder
    /// stages generalize on their own, undiluted by a filter tuned for
    /// exactly their shape.
    #[must_use]
    pub const fn sealed_only(self) -> bool {
        matches!(self, Self::AccessLog | Self::GradientImage)
    }
}

/// Regret score for a candidate corpus addition
/// (`research/corpus/POLICY.md`, "Growing the corpus": "regret = (our
/// bits/byte) − (reference compressor bits/byte) on the same data.
/// Additions need positive regret — data we are *relatively* bad at.").
///
/// `ours_bpb` is mothergod's bits/byte on the candidate data; `zstd_bpb`
/// and `xz_bpb` are the two pinned reference compressors' bits/byte
/// (`zstd -19`, `xz -9e`) on the same data. Regret is measured against
/// whichever reference does better here, so a data class only counts as
/// "we're relatively bad at this" when we lose to the stronger of the
/// two, not just the weaker one.
///
/// Positive regret is the accept criterion. Policy also auto-rejects pure
/// noise, which needs no separate case here: noise is equally
/// incompressible for every compressor, so `ours_bpb`, `zstd_bpb`, and
/// `xz_bpb` all sit near 8 bits/byte and regret comes out near zero,
/// already failing the positive-regret test.
#[must_use]
pub fn regret(ours_bpb: f64, zstd_bpb: f64, xz_bpb: f64) -> f64 {
    ours_bpb - zstd_bpb.min(xz_bpb)
}

#[cfg(test)]
mod tests {
    use super::{
        DatasetKind, SQLITE_ROW_WIDTH, access_log, base64_encode, base64_wrapped, entropy_ladder,
        gradient_image, interleaved_audio16, json_records, markov_h8_2_trap, order0_entropy_bits,
        order1_conditional_entropy_bits, regret, sealed_seed, sqlite_like_records, train_window,
        x86_dense_code,
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
        assert_eq!(json_records(0, SEED), Vec::<u8>::new());
        assert_eq!(base64_wrapped(0, SEED), Vec::<u8>::new());
        assert_eq!(interleaved_audio16(0, SEED), Vec::<u8>::new());
        assert_eq!(gradient_image(0, SEED), Vec::<u8>::new());
        assert_eq!(sqlite_like_records(0, SEED), Vec::<u8>::new());
        assert_eq!(x86_dense_code(0, SEED), Vec::<u8>::new());
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
    fn json_records_is_exactly_the_requested_length() {
        for len in [1, 2, 47, 1000, LEN] {
            assert_eq!(json_records(len, SEED).len(), len);
        }
    }

    #[test]
    fn json_records_is_deterministic() {
        assert_eq!(json_records(5_000, SEED), json_records(5_000, SEED));
    }

    #[test]
    fn json_records_seeds_are_independent() {
        assert_ne!(json_records(5_000, SEED), json_records(5_000, SEED + 1));
    }

    #[test]
    fn json_records_looks_like_json_records() {
        let data = json_records(LEN, SEED);
        let text = String::from_utf8(data).expect("generator only emits ASCII");
        assert!(text.starts_with(r#"{"status": "ok", "results": ["#));
        let record_count = text.matches("\"user_id\"").count();
        assert!(
            record_count > 100,
            "expected many records, got {record_count}"
        );
    }

    #[test]
    fn json_records_active_field_is_skewed_true() {
        let text = String::from_utf8(json_records(LEN, SEED)).expect("ASCII only");
        let true_count = text.matches("\"active\": true").count();
        let false_count = text.matches("\"active\": false").count();
        let total = true_count + false_count;
        assert!(total > 100, "expected many records");
        // ~80% true; a 70-90% band avoids a flaky exact-match assertion,
        // checked with integer arithmetic to sidestep a precision-loss cast.
        assert!(
            true_count * 10 > total * 7 && true_count * 10 < total * 9,
            "expected ~80% active=true, got {true_count}/{total}"
        );
    }

    #[test]
    fn base64_encode_matches_rfc_4648_test_vectors() {
        // https://www.rfc-editor.org/rfc/rfc4648#section-10
        assert_eq!(base64_encode(b""), b"");
        assert_eq!(base64_encode(b"f"), b"Zg==");
        assert_eq!(base64_encode(b"fo"), b"Zm8=");
        assert_eq!(base64_encode(b"foo"), b"Zm9v");
        assert_eq!(base64_encode(b"foob"), b"Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), b"Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), b"Zm9vYmFy");
    }

    #[test]
    fn base64_wrapped_is_exactly_the_requested_length() {
        for len in [1, 2, 47, 1000, LEN] {
            assert_eq!(base64_wrapped(len, SEED).len(), len);
        }
    }

    #[test]
    fn base64_wrapped_is_deterministic() {
        assert_eq!(base64_wrapped(5_000, SEED), base64_wrapped(5_000, SEED));
    }

    #[test]
    fn base64_wrapped_seeds_are_independent() {
        assert_ne!(base64_wrapped(5_000, SEED), base64_wrapped(5_000, SEED + 1));
    }

    #[test]
    fn base64_wrapped_is_all_base64_alphabet_bytes() {
        let data = base64_wrapped(LEN, SEED);
        assert!(
            data.iter()
                .all(|&b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'='),
            "expected only standard base64 alphabet bytes"
        );
    }

    #[test]
    fn interleaved_audio16_is_exactly_the_requested_length() {
        for len in [1, 2, 3, 47, 1000, LEN] {
            assert_eq!(interleaved_audio16(len, SEED).len(), len);
        }
    }

    #[test]
    fn interleaved_audio16_is_deterministic() {
        assert_eq!(
            interleaved_audio16(5_000, SEED),
            interleaved_audio16(5_000, SEED)
        );
    }

    #[test]
    fn interleaved_audio16_seeds_are_independent() {
        assert_ne!(
            interleaved_audio16(5_000, SEED),
            interleaved_audio16(5_000, SEED + 1)
        );
    }

    #[test]
    fn interleaved_audio16_samples_move_smoothly() {
        // A sine-plus-noise signal changes little from one 16-bit sample to
        // the next, unlike iid random data: consecutive samples (as signed
        // i16, little-endian) should mostly differ by far less than the
        // full ~65536 range.
        let data = interleaved_audio16(LEN, SEED);
        let samples: Vec<i16> = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| i16::from_le_bytes(pair))
            .collect();
        let small_steps = samples
            .windows(2)
            .filter(|pair| i32::from(pair[1]).abs_diff(i32::from(pair[0])) < 2000)
            .count();
        assert!(
            small_steps * 10 > samples.len() * 9,
            "expected >90% of consecutive samples to differ by under 2000, got {small_steps}/{}",
            samples.len()
        );
    }

    #[test]
    fn gradient_image_is_exactly_the_requested_length() {
        for len in [1, 2, 3, 47, 1000, LEN] {
            assert_eq!(gradient_image(len, SEED).len(), len);
        }
    }

    #[test]
    fn gradient_image_is_deterministic() {
        assert_eq!(gradient_image(5_000, SEED), gradient_image(5_000, SEED));
    }

    #[test]
    fn gradient_image_seeds_are_independent() {
        assert_ne!(gradient_image(5_000, SEED), gradient_image(5_000, SEED + 1));
    }

    #[test]
    fn gradient_image_pixels_move_smoothly_along_a_row() {
        // A sine-plus-noise gradient changes little from one pixel to the
        // next within a row, unlike iid random data.
        let data = gradient_image(LEN, SEED);
        let small_steps = data
            .windows(2)
            .filter(|pair| i32::from(pair[1]).abs_diff(i32::from(pair[0])) < 40)
            .count();
        assert!(
            small_steps * 10 > (data.len() - 1) * 9,
            "expected >90% of consecutive pixels to differ by under 40, got {small_steps}/{}",
            data.len() - 1
        );
    }

    #[test]
    fn sqlite_like_records_is_exactly_the_requested_length() {
        for len in [1, 2, 3, 47, 1000, LEN] {
            assert_eq!(sqlite_like_records(len, SEED).len(), len);
        }
    }

    #[test]
    fn sqlite_like_records_is_deterministic() {
        assert_eq!(
            sqlite_like_records(5_000, SEED),
            sqlite_like_records(5_000, SEED)
        );
    }

    #[test]
    fn sqlite_like_records_seeds_are_independent() {
        assert_ne!(
            sqlite_like_records(5_000, SEED),
            sqlite_like_records(5_000, SEED + 1)
        );
    }

    #[test]
    fn sqlite_like_records_uses_only_the_fixed_category_set() {
        // Every row's category field (bytes 8..12) must be one of the three
        // fixed values; a category field is 4-byte-aligned data with far
        // fewer distinct values than an iid source would produce.
        let data = sqlite_like_records(LEN, SEED);
        let allowed: [&[u8]; 3] = [b"temp", b"hum\0", b"pres"];
        let full_rows = data.len() / SQLITE_ROW_WIDTH;
        for row in 0..full_rows {
            let base = row * SQLITE_ROW_WIDTH;
            let category = &data[base + 8..base + 12];
            assert!(
                allowed.contains(&category),
                "row {row} has an unexpected category field: {category:?}"
            );
        }
    }

    #[test]
    fn sqlite_like_records_timestamps_increase_monotonically() {
        let data = sqlite_like_records(LEN, SEED);
        let full_rows = data.len() / SQLITE_ROW_WIDTH;
        let mut prev: Option<i64> = None;
        for row in 0..full_rows {
            let base = row * SQLITE_ROW_WIDTH;
            let ts = i64::from_le_bytes(data[base..base + 8].try_into().unwrap());
            if let Some(p) = prev {
                assert!(ts > p, "row {row} timestamp {ts} did not increase past {p}");
            }
            prev = Some(ts);
        }
    }

    #[test]
    fn x86_dense_code_is_exactly_the_requested_length() {
        for len in [1, 2, 3, 4, 5, 6, 47, 1000, LEN] {
            assert_eq!(x86_dense_code(len, SEED).len(), len);
        }
    }

    #[test]
    fn x86_dense_code_is_deterministic() {
        assert_eq!(x86_dense_code(5_000, SEED), x86_dense_code(5_000, SEED));
    }

    #[test]
    fn x86_dense_code_seeds_are_independent() {
        assert_ne!(x86_dense_code(5_000, SEED), x86_dense_code(5_000, SEED + 1));
    }

    #[test]
    fn x86_dense_code_is_dense_with_call_and_jmp_opcodes() {
        // "Dense" is the point of this class: a real binary's call/jmp
        // share is far lower than this, but the class exists to stress the
        // bcj filter, so demand a healthy floor rather than a realistic one.
        let data = x86_dense_code(LEN, SEED);
        let opcode_bytes = data.iter().filter(|&&b| b == 0xE8 || b == 0xE9).count();
        assert!(
            opcode_bytes * 20 > data.len(),
            "expected >5% of bytes to be call/jmp opcodes, got {opcode_bytes}/{}",
            data.len()
        );
    }

    #[test]
    fn x86_dense_code_round_trips_through_the_bcj_filter() {
        // The class exists to exercise `bcj` (S2-A4); confirm it actually
        // does, independent of the frame-format round trip below.
        let data = x86_dense_code(LEN, SEED);
        let filtered = mothergod::filters::bcj::encode(&data);
        assert_eq!(mothergod::filters::bcj::decode(&filtered), data);
    }

    #[test]
    fn generators_round_trip_through_the_frame_format() {
        for data in [
            entropy_ladder(1, 5_000, SEED),
            entropy_ladder(8, 5_000, SEED),
            markov_h8_2_trap(5_000, SEED),
            access_log(5_000, SEED),
            json_records(5_000, SEED),
            base64_wrapped(5_000, SEED),
            interleaved_audio16(5_000, SEED),
            gradient_image(5_000, SEED),
            sqlite_like_records(5_000, SEED),
            x86_dense_code(5_000, SEED),
        ] {
            assert_eq!(mothergod::decompress(&mothergod::compress(&data)), Ok(data));
        }
    }

    #[test]
    fn train_window_is_exactly_the_requested_length() {
        let data: Vec<u8> = (0..100u8).collect();
        for window_len in [1, 7, 50, 99, 100] {
            for iteration in [0, 1, 50, 100, 1_000, u64::MAX] {
                assert_eq!(train_window(&data, window_len, iteration).len(), window_len);
            }
        }
    }

    #[test]
    fn train_window_at_iteration_zero_starts_at_the_front() {
        let data: Vec<u8> = (0..100u8).collect();
        assert_eq!(train_window(&data, 10, 0), data[0..10].to_vec());
    }

    #[test]
    fn train_window_slides_without_wrapping_when_it_fits() {
        let data: Vec<u8> = (0..100u8).collect();
        assert_eq!(train_window(&data, 10, 5), data[5..15].to_vec());
    }

    #[test]
    fn train_window_wraps_circularly_past_the_end() {
        let data: Vec<u8> = (0..100u8).collect();
        // Starting at offset 95 with a 10-byte window runs off the end at
        // 105; the last 5 bytes must wrap back to the front.
        let window = train_window(&data, 10, 95);
        assert_eq!(window, [95, 96, 97, 98, 99, 0, 1, 2, 3, 4]);
    }

    #[test]
    fn train_window_repeats_after_one_full_cycle_of_data_len() {
        let data: Vec<u8> = (0..100u8).collect();
        for iteration in [0, 1, 37, 99] {
            assert_eq!(
                train_window(&data, 10, iteration),
                train_window(&data, 10, iteration + data.len() as u64)
            );
        }
    }

    #[test]
    fn train_window_consecutive_iterations_differ_when_rotation_is_possible() {
        let data: Vec<u8> = (0..100u8).collect();
        assert_ne!(train_window(&data, 10, 0), train_window(&data, 10, 1));
    }

    #[test]
    fn train_window_whole_buffer_still_rotates_by_iteration() {
        // A window as long as the whole buffer still rotates: iteration 0
        // is the buffer as-is, iteration 1 is rotated left by one byte.
        let data: Vec<u8> = (0..100u8).collect();
        assert_eq!(train_window(&data, 100, 0), data);
        let mut rotated = data[1..].to_vec();
        rotated.push(data[0]);
        assert_eq!(train_window(&data, 100, 1), rotated);
    }

    #[test]
    #[should_panic(expected = "window_len must be nonzero")]
    fn train_window_rejects_a_zero_length_window() {
        let data: Vec<u8> = (0..10u8).collect();
        let _ = train_window(&data, 0, 0);
    }

    #[test]
    #[should_panic(expected = "window_len exceeds data.len()")]
    fn train_window_rejects_a_window_longer_than_the_data() {
        let data: Vec<u8> = (0..10u8).collect();
        let _ = train_window(&data, 11, 0);
    }

    #[test]
    fn sealed_seed_is_deterministic() {
        assert_eq!(sealed_seed(0), sealed_seed(0));
        assert_eq!(sealed_seed(0x00C0_FFEE), sealed_seed(0x00C0_FFEE));
    }

    #[test]
    fn sealed_seed_differs_from_its_train_seed() {
        for seed in [0, 1, 42, 0x00C0_FFEE, 0xC0FF_EE12_3456_789A_u64, u64::MAX] {
            assert_ne!(sealed_seed(seed), seed);
        }
    }

    #[test]
    fn sealed_seed_is_injective_over_a_swept_range() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..10_000u64 {
            assert!(seen.insert(sealed_seed(seed)), "collision at seed {seed}");
        }
    }

    #[test]
    fn sealed_seed_is_injective_over_train_and_sealed_seeds_together() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..10_000u64 {
            seen.insert(seed);
        }
        for seed in 0..10_000u64 {
            assert!(
                seen.insert(sealed_seed(seed)),
                "sealed_seed({seed}) collided with a plain train seed in [0, 10_000)"
            );
        }
    }

    #[test]
    fn dataset_kind_all_has_no_duplicates() {
        let seen: std::collections::HashSet<_> = DatasetKind::ALL.iter().collect();
        assert_eq!(seen.len(), DatasetKind::ALL.len());
    }

    #[test]
    fn dataset_kind_mandatory_kinds_stay_in_train() {
        assert!(!DatasetKind::EntropyLadder.sealed_only());
        assert!(!DatasetKind::MarkovH82Trap.sealed_only());
    }

    #[test]
    fn dataset_kind_sealed_only_is_a_proper_nonempty_subset_of_all() {
        let sealed_only: Vec<_> = DatasetKind::ALL
            .into_iter()
            .filter(|kind| kind.sealed_only())
            .collect();
        assert!(!sealed_only.is_empty(), "nothing is held out for sealed");
        assert!(
            sealed_only.len() < DatasetKind::ALL.len(),
            "every kind is sealed-only, so train has nothing to tune against"
        );
    }

    #[test]
    fn regret_is_zero_when_ours_matches_the_stronger_reference() {
        assert!((regret(2.0, 2.0, 2.5) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn regret_is_positive_when_ours_loses_to_both_references() {
        assert!((regret(3.0, 2.0, 2.5) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn regret_is_negative_when_ours_beats_both_references() {
        assert!((regret(1.0, 2.0, 2.5) - -1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn regret_is_symmetric_in_which_reference_arg_is_stronger() {
        assert!((regret(3.0, 2.0, 2.5) - regret(3.0, 2.5, 2.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn regret_is_near_zero_on_pure_noise() {
        // All three compressors sit near the incompressible floor: no one
        // wins, so regret should not mistake this for a data class we are
        // relatively bad at.
        let ours = 8.0;
        let zstd = 8.02;
        let xz = 8.01;
        assert!(
            regret(ours, zstd, xz).abs() < 0.1,
            "pure noise should score near-zero regret"
        );
    }
}
