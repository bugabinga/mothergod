/// Deterministic pseudo-random symbol stream for round-trip tests in
/// `coder`, `model`, and `literal`: those three modules' test suites
/// each need a long, deterministic-but-unstructured stream with no
/// external RNG dependency, and had each hand-rolled the same
/// xorshift32 step to get one.
///
/// xorshift32 generator: `next()` advances the state and returns it,
/// so the seed itself is never yielded, only states derived from it.
pub(crate) struct Xorshift32(u32);

impl Xorshift32 {
    pub(crate) fn new(seed: u32) -> Self {
        Self(seed)
    }
}

impl Iterator for Xorshift32 {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        Some(self.0)
    }
}

/// `v` as a [`std::num::NonZeroUsize`], for tests that need one as a
/// filter parameter (a delta stride, a transpose column count) and
/// know `v` is nonzero by construction: `filters::delta` and
/// `filters::transpose`'s test suites each need this same conversion
/// and had each hand-rolled the identical helper to get it.
///
/// # Panics
///
/// Panics if `v` is zero: every call site passes a literal already
/// known to be nonzero, so this is a test-fixture bug, never
/// something a non-test caller could trigger (this function only
/// exists under `#[cfg(test)]`).
pub(crate) fn nz(v: usize) -> std::num::NonZeroUsize {
    std::num::NonZeroUsize::new(v).unwrap()
}

/// Extracts the [`Error`] from a [`WriteError`] produced by a
/// `decode_to_writer` call, for tests asserting exactly which decode
/// error occurred; `None` for a [`WriteError::Io`] (a `writer` failure,
/// never a decode error): `codec` and `lib`'s test suites each need
/// this same match and had each hand-rolled the identical helper to
/// get it.
pub(crate) fn as_codec_error(err: &super::WriteError) -> Option<&super::Error> {
    match err {
        super::WriteError::Decode(inner) => Some(inner),
        super::WriteError::Io(_) => None,
    }
}
