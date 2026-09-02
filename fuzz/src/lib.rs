//! Support code shared across `fuzz/fuzz_targets/`, kept out of that
//! directory because cargo-fuzz treats every file there as its own
//! `#![no_main]` binary rather than an importable module.

pub mod frame_gen;
