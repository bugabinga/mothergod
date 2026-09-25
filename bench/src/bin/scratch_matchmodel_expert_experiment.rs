//! Uncommitted scratch driver for `research/JOURNAL.md` S1-P8's hash-based
//! match-model expert slice: measures
//! `mothergod::codec::ideal_cost_bits_matchmodel_expert_experiment` over
//! `bench::baseline`'s 11 train-tier cases plus both sealed-only kinds at
//! `sealed_seed(CASE_SEED)`, matching S2-A100/S2-R20's own convention
//! exactly. Deleted after this measurement is recorded in the journal, per
//! the `compression-experiment` skill.
//!
//! Usage: `cargo run -p mothergod-bench --release --bin
//! scratch_matchmodel_expert_experiment`.

use mothergod::codec::ideal_cost_bits_matchmodel_expert_experiment;
use mothergod_bench::baseline::{CASE_LEN, CASE_SEED, cases};
use mothergod_bench::{access_log, gradient_image, sealed_seed};

fn report(name: &str, data: &[u8]) -> f64 {
    let (baseline_bits, candidate_bits) = ideal_cost_bits_matchmodel_expert_experiment(data);
    #[allow(clippy::cast_precision_loss, reason = "scratch measurement binary")]
    let len = data.len() as f64;
    let delta_bpb = (candidate_bits - baseline_bits) / len;
    println!(
        "{name:24} baseline={baseline_bits:12.3} candidate={candidate_bits:12.3} delta_bpb={delta_bpb:+.6}"
    );
    delta_bpb
}

fn main() {
    println!("train cases (CASE_LEN={CASE_LEN}, CASE_SEED={CASE_SEED:#x}):");
    let mut train_total = 0.0;
    let mut train_count = 0;
    for case in cases() {
        train_total += report(case.name, &case.data);
        train_count += 1;
    }
    println!(
        "train net delta_bpb (mean over {train_count} cases): {:+.6}",
        train_total / f64::from(train_count)
    );

    println!("\nsealed-only cases (sealed_seed(CASE_SEED)):");
    let sealed = sealed_seed(CASE_SEED);
    report("access_log", &access_log(CASE_LEN, sealed));
    report("gradient_image", &gradient_image(CASE_LEN, sealed));
}
