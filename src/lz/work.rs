//! Test-only work counters: bytes compared by the two scans, and candidates
//! visited by the tree walk, whose cost the complexity guards in `tests`
//! bound. A guard that bounds a count asserts the property (no quadratic
//! rescans) and holds on any runner; one that bounds a stopwatch asserts a
//! hardware proxy of it, and failed under `cargo llvm-cov` on 2026-10-11
//! (issue #1059).
//!
//! Thread-local, because the test harness runs each test on its own thread
//! and the parse spawns none: one test's count never sees another's.

use std::cell::Cell;

thread_local! {
    static MATCH_LEN: Cell<u64> = const { Cell::new(0) };
    static SUFFIX_COMMON_LEN: Cell<u64> = const { Cell::new(0) };
    static CANDIDATES: Cell<u64> = const { Cell::new(0) };
}

/// A counted unit of work: bytes for the two scans, candidates for the walk.
#[derive(Clone, Copy)]
pub(super) enum Scan {
    /// [`super::match_len`], the rep-candidate and greedy-parse scan.
    MatchLen,
    /// [`super::suffix_common_len`], the binary tree's candidate compare.
    SuffixCommonLen,
    /// Candidates [`super::BinaryTreeMatchFinder::insert_and_find`] visited,
    /// one per node it compared. Counted apart from `SuffixCommonLen`
    /// because a candidate whose compare starts at the proven common
    /// length costs no bytes, so bytes cannot see a walk that fails to stop.
    Candidates,
}

impl Scan {
    fn cell(self) -> &'static std::thread::LocalKey<Cell<u64>> {
        match self {
            Self::MatchLen => &MATCH_LEN,
            Self::SuffixCommonLen => &SUFFIX_COMMON_LEN,
            Self::Candidates => &CANDIDATES,
        }
    }

    /// Adds `units` of work (see [`Scan`]) to this thread's count.
    pub(super) fn add(self, units: usize) {
        self.cell().with(|c| c.set(c.get() + units as u64));
    }

    /// Units of work on this thread since it last called [`Self::reset`].
    pub(super) fn count(self) -> u64 {
        self.cell().with(Cell::get)
    }

    /// Zeroes this thread's count.
    pub(super) fn reset(self) {
        self.cell().with(|c| c.set(0));
    }
}
