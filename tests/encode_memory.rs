//! The encoder's peak heap, per input byte, on input that parses into many
//! tokens (`research/JOURNAL.md` S2-A124).
//!
//! The optimal parse runs its DP rounds one after another, and a round's
//! token list is dead once the next round's prices are tallied from it.
//! Nothing in the output shows whether it was freed: the bytes are the same
//! either way, and only the peak differs, by the list's capacity, up to
//! 12 bytes for each token the doubling `Vec` has room for. A zeros input
//! parses into a handful of tokens and cannot see it, so this one is
//! words: a few hundred distinct ones in a pseudo-random order.
//!
//! One test in its own binary, because the allocator counts every thread:
//! a second test running beside it would be in the peak.

use std::alloc::{GlobalAlloc, Layout, System};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};

struct PeakAlloc;

// Relaxed throughout: the peak is read after `compress` returns, on the one
// thread that ran it, and the counters synchronize nothing else.
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

fn shrink(bytes: usize) {
    LIVE.fetch_sub(bytes, Ordering::Relaxed);
}

// SAFETY: every method forwards to `System`, whose `GlobalAlloc` impl is
// trusted, and counts only what `System` returned: a null from a failed
// allocation is not counted.
unsafe impl GlobalAlloc for PeakAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            grow(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        shrink(layout.size());
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            grow(layout.size());
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(ptr, layout, new_size) };
        if !moved.is_null() {
            shrink(layout.size());
            grow(new_size);
        }
        moved
    }
}

#[global_allocator]
static ALLOC: PeakAlloc = PeakAlloc;

/// `len` bytes of xorshift32 noise: no match to find, so every position is
/// a literal token and the token lists are as long as they ever get.
fn noise(len: usize) -> Vec<u8> {
    let mut state = 0x2545_f491_u32;
    std::iter::repeat_with(|| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state.to_le_bytes()[1]
    })
    .take(len)
    .collect()
}

/// Bytes of peak heap per input byte one encode of [`noise`] may reach. The
/// peak measures 37.6 with each round's tokens freed before the next DP round
/// and 49.6 without, on the 64-bit allocator this runs under; the bound sits
/// between, so the test fails on the regression and not on a small drift.
const MAX_PEAK_PER_BYTE: usize = 44;

/// `codec::encode` of `input` with `workers` trials at once, and the peak
/// heap it reached, rounded up to bytes per input byte. Each running trial
/// holds an encode's working set of its own (`research/JOURNAL.md` S2-A126).
fn encode_peak(input: &[u8], workers: usize) -> (usize, Vec<u8>) {
    let before = LIVE.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let workers = NonZeroUsize::new(workers).expect("a worker count is nonzero");
    let payload = mothergod::codec::encode(input, workers);
    let peak = PEAK.load(Ordering::Relaxed) - before;
    (peak.div_ceil(input.len()), payload)
}

#[test]
fn the_peak_heap_of_an_encode_stays_under_its_bytes_per_input_byte() {
    // Under 512 KiB the peak is the models and tables, which do not grow with
    // the input, and the token lists are not in it.
    const LEN: usize = 1 << 19;
    let input = noise(LEN);
    let (serial_peak, serial) = encode_peak(&input, 1);
    assert!(
        serial_peak < MAX_PEAK_PER_BYTE,
        "peak heap {serial_peak} bytes per input byte of {LEN}, over {MAX_PEAK_PER_BYTE}"
    );
    // A pair of trials is two working sets and no more: a third at once is
    // what the bound the CLI states does not hold.
    let (pair_peak, pair) = encode_peak(&input, 2);
    assert!(
        pair_peak < 2 * MAX_PEAK_PER_BYTE,
        "peak heap {pair_peak} bytes per input byte of {LEN} with two trials at once, over {}",
        2 * MAX_PEAK_PER_BYTE
    );
    assert_eq!(pair, serial);
    assert_eq!(
        mothergod::codec::decode(&serial, mothergod::codec::MAX_DECODED_LEN),
        Ok(input)
    );
}
