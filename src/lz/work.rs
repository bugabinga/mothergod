//! Test-only work counters: bytes compared by the two scans whose cost the
//! complexity guards in `tests` bound. A guard that bounds a count asserts
//! the property (no quadratic rescans) and holds on any runner; one that
//! bounds a stopwatch asserts a hardware proxy of it, and failed under
//! `cargo llvm-cov` on 2026-10-11 (issue #1059).
//!
//! Thread-local, because the test harness runs each test on its own thread
//! and the parse spawns none: one test's count never sees another's.

use std::cell::Cell;

thread_local! {
    static MATCH_LEN: Cell<u64> = const { Cell::new(0) };
    static SUFFIX_COMMON_LEN: Cell<u64> = const { Cell::new(0) };
}

/// A scan whose bytes are counted.
#[derive(Clone, Copy)]
pub(super) enum Scan {
    /// [`super::match_len`], the rep-candidate and greedy-parse scan.
    MatchLen,
    /// [`super::suffix_common_len`], the binary tree's candidate compare.
    SuffixCommonLen,
}

impl Scan {
    fn cell(self) -> &'static std::thread::LocalKey<Cell<u64>> {
        match self {
            Self::MatchLen => &MATCH_LEN,
            Self::SuffixCommonLen => &SUFFIX_COMMON_LEN,
        }
    }

    /// Adds `bytes` compared to this thread's count.
    pub(super) fn add(self, bytes: usize) {
        self.cell().with(|c| c.set(c.get() + bytes as u64));
    }

    /// Bytes compared on this thread since it last called [`Self::reset`].
    pub(super) fn count(self) -> u64 {
        self.cell().with(Cell::get)
    }

    /// Zeroes this thread's count.
    pub(super) fn reset(self) {
        self.cell().with(|c| c.set(0));
    }
}
