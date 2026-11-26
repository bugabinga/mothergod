//! Entry point for [`mothergod_bench::bit_mix::rate_vs_champion`], charter
//! #1922's class (b) rate driver (issue #2035): the champion's reading and
//! the try's come from two builds, so two processes, and this binary
//! carries the first to the second as a file.
//!
//! `measure` reads the rate once and prints its
//! [`RateReport::record`] on stdout, the human report on stderr, so
//! `measure > <file>` on the champion's build keeps the reading.
//! `verdict <file>` reads the rate on the try's build, prints its report,
//! both ratios with their margins, and
//! [`RateReport::falls_past_margin`] against the file's reading:
//! `Some(true)` keeps the try, `Some(false)` does not, `None` is two CPU
//! models and no verdict. It exits 0 whatever the verdict, because it
//! reports, and 1 when the file is no record or its literal bytes differ
//! from the try's, since the two readings then walked different streams.
//! The file is read before the rate is, so a refusal of it costs no
//! measurement.
//!
//! Usage, in a release build, both on one CPU model: `cargo run -p
//! mothergod-bench --release --bin bit_mix_rate -- measure > <file>` on
//! the champion, then `... -- verdict <file>` on the try.

use mothergod_bench::bit_mix::{RateReport, rate_vs_champion};
use std::process::ExitCode;

const USAGE: &str = "usage: bit_mix_rate measure > <file> | bit_mix_rate verdict <file>";

/// The `verdict` mode's report: the try's reading, then both ratios and
/// the keep rule's answer, or `Err` when the two readings walked
/// different literal streams.
fn verdict(before: &RateReport, after: &RateReport) -> Result<String, String> {
    if before.literal_bytes != after.literal_bytes {
        return Err(format!(
            "the champion's reading walked {} literal bytes and the try's {}, \
             so the ratios time different streams",
            before.literal_bytes, after.literal_bytes
        ));
    }
    Ok(format!(
        "{after}\nchampion's build: ratio {:.4}, margin {:.1}%\n\
         try's build: ratio {:.4}, margin {:.1}%\n\
         falls_past_margin: {:?}",
        before.ratio(),
        before.margin() * 100.0,
        after.ratio(),
        after.margin() * 100.0,
        after.falls_past_margin(before)
    ))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["measure"] => {
            let reading = rate_vs_champion();
            eprintln!("{reading}");
            print!("{}", reading.record());
            ExitCode::SUCCESS
        }
        ["verdict", path] => {
            let before = std::fs::read_to_string(path)
                .map_err(|err| err.to_string())
                .and_then(|text| RateReport::from_record(&text));
            let report = before.and_then(|before| verdict(&before, &rate_vs_champion()));
            match report {
                Ok(report) => {
                    println!("{report}");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("{path}: {err}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mothergod_bench::bit_mix::RateRound;

    fn reading(literal_bytes: usize, candidate: f64) -> RateReport {
        RateReport {
            cpu: Some("Xeon 6975P-C".to_string()),
            literal_bytes,
            calibration_seconds: 4.0,
            rounds: vec![RateRound {
                champion: 1.0,
                candidate,
            }],
        }
    }

    #[test]
    fn a_verdict_prints_both_ratios_and_the_keep_rule_s_answer() {
        let report = verdict(&reading(1_000, 0.30), &reading(1_000, 0.25)).expect("one stream");
        assert!(
            report.ends_with(
                "champion's build: ratio 0.3000, margin 8.0%\n\
                 try's build: ratio 0.2500, margin 8.0%\n\
                 falls_past_margin: Some(true)"
            ),
            "{report}"
        );
    }

    #[test]
    fn readings_over_different_literal_bytes_give_no_verdict() {
        assert_eq!(
            verdict(&reading(1_000, 0.30), &reading(999, 0.25)),
            Err(
                "the champion's reading walked 1000 literal bytes and the try's 999, \
                 so the ratios time different streams"
                    .to_string()
            )
        );
    }
}
