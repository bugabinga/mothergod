//! Scratch: real-bitstream deltas for ADR-0054's wiring slice, against
//! `bench/baseline.json`'s committed (S2-A102/`LogisticMix`-era) numbers.
//! Deleted once the numbers are recorded (`compression-experiment` skill).

use mothergod_bench::baseline::{bits_per_byte, cases, parse_baseline, CASE_LEN, CASE_SEED};
use mothergod_bench::{access_log, gradient_image, sealed_seed};
use std::fs;

fn main() {
    let baseline_path = concat!(env!("CARGO_MANIFEST_DIR"), "/baseline.json");
    let committed = parse_baseline(&fs::read_to_string(baseline_path).unwrap()).unwrap();

    let mut sum_delta = 0.0;
    for case in cases() {
        let compressed = mothergod::compress(&case.data);
        let measured = bits_per_byte(compressed.len(), case.data.len());
        let before = committed[case.name];
        let delta = measured - before;
        sum_delta += delta;
        println!("{:<24} {before:.6} -> {measured:.6} ({delta:+.6})", case.name);
    }
    println!("train mean delta: {:+.6}", sum_delta / cases().len() as f64);

    for (name, data) in [
        ("access_log", access_log(CASE_LEN, sealed_seed(CASE_SEED))),
        (
            "gradient_image",
            gradient_image(CASE_LEN, sealed_seed(CASE_SEED)),
        ),
    ] {
        let compressed = mothergod::compress(&data);
        let measured = bits_per_byte(compressed.len(), data.len());
        println!("sealed {name:<16} = {measured:.6}");
    }
}
