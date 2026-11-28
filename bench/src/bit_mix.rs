//! Charter #1922's literal coder class (b),
//! [`mothergod::literal::bit_mix::BitMix`], priced by
//! [`crate::literal_pairing`] against the default level's literal model,
//! [`Literal::walk_logit_sse`], on the parse's literal stream
//! (`research/JOURNAL.md` S2-A159). Both sides are ideal cost.
//!
//! [`rate_vs_champion`] is the class's rate driver: it times both walks
//! over the train cases' literal streams in one process and states its own
//! margin, so a rate try is kept on a reading a review re-runs
//! (`research/JOURNAL.md` S2-A161). S2-A160's speed reading came from a
//! scratch clock that left with its session.

use crate::baseline;
use crate::cm_path::cost_bits;
use crate::literal_pairing::{self, PairingReport, literal_replay};
use crate::reference::cpu_model;
use crate::speed::{self, Kernel, median_of_sorted};
use mothergod::literal::bit_mix::BitMix;
use mothergod::literal::{Context, Literal, SurpriseLogisticMix};
use std::fmt::{self, Write as _};
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::time::Instant;

/// The champion's pricer: [`Literal::walk_logit_sse`] under a fresh
/// [`SurpriseLogisticMix`], the walk every non-transposed `Lz` frame's
/// literals take.
pub fn champion() -> impl FnMut(&mut Literal, Context, u8) -> f64 {
    let mut mixer = SurpriseLogisticMix::new();
    move |model: &mut Literal, context, byte| {
        let symbol = usize::from(byte);
        let mut bits = 0.0;
        model.walk_logit_sse(context, &mut mixer, |mid, p| {
            let bit = symbol >= mid;
            bits += cost_bits(bit, p);
            bit
        });
        bits
    }
}

/// The candidate's pricer: a fresh [`BitMix`], ignoring the [`Literal`]
/// [`literal_pairing::literal_replay`] hands it.
pub fn candidate() -> impl FnMut(&mut Literal, Context, u8) -> f64 {
    let mut model = BitMix::new();
    move |_: &mut Literal, context, byte| {
        let symbol = usize::from(byte);
        let mut bits = 0.0;
        model.walk(context, |mid, p| {
            let bit = symbol >= mid;
            bits += cost_bits(bit, p);
            bit
        });
        bits
    }
}

/// The recorded experiment: [`candidate`] against [`champion`] over the
/// eleven train cases and both sealed kinds.
#[must_use]
pub fn train_and_sealed_delta_bpb() -> PairingReport {
    literal_pairing::train_and_sealed_delta_bpb(champion, candidate)
}

/// One case's literal stream in coding order, each literal's context and
/// byte, recorded once from the parse so a timed walk pays for no parse.
#[must_use]
pub fn literal_stream(data: &[u8]) -> Vec<(Context, u8)> {
    let mut stream = Vec::new();
    literal_replay(data, |_: &mut Literal, context, byte| {
        stream.push((context, byte));
        0.0
    });
    stream
}

/// A literal model as the rate driver times it: built fresh per case, then
/// walked once per literal, the literal's own byte resolving every node, as
/// a decoder's range coder would.
trait RateWalk {
    /// A fresh model, as a stream's first literal finds it.
    fn fresh() -> Self;

    /// One byte's walk under `context`, resolved to `byte`; returns the
    /// landed byte.
    fn walk_byte(&mut self, context: Context, byte: u8) -> u8;
}

/// The bit `byte` resolves a node at `mid` to. The probability goes
/// through [`black_box`]: a decoder reads it in its range coder, and a walk
/// that discards it lets the compiler delete the work that computes it, so
/// the clock would time a walk no decoder runs.
///
/// The bit also depends on `p` in the data, as a decoder's does: its range
/// coder compares against `p`, and the next node's slot index hangs on the
/// bit, so one node's work waits on the last. A bit read from `byte` alone
/// lets the core overlap neighbouring nodes and times the walk's throughput,
/// which under-read the class (b) walk's decode time
/// (`research/JOURNAL.md` S2-A166). `p` is never NaN, so the `xor` changes
/// no bit; the compiler cannot prove that, so it keeps the dependency.
fn resolve(byte: u8, mid: usize, p: f64) -> bool {
    black_box(p);
    (usize::from(byte) >= mid) ^ p.is_nan()
}

/// [`champion`]'s walk without its pricing.
struct ChampionWalk {
    model: Literal,
    mixer: SurpriseLogisticMix,
}

impl RateWalk for ChampionWalk {
    fn fresh() -> Self {
        Self {
            model: Literal::new(),
            mixer: SurpriseLogisticMix::new(),
        }
    }

    fn walk_byte(&mut self, context: Context, byte: u8) -> u8 {
        self.model
            .walk_logit_sse(context, &mut self.mixer, |mid, p| resolve(byte, mid, p))
    }
}

impl RateWalk for BitMix {
    fn fresh() -> Self {
        Self::new()
    }

    fn walk_byte(&mut self, context: Context, byte: u8) -> u8 {
        self.walk(context, |mid, p| resolve(byte, mid, p))
    }
}

/// `M`'s walk time over `streams`, each stream best of
/// [`speed::REPEATS`] from a fresh model, summed: [`speed::decode_seconds`]'s
/// shape. A model is built outside the timer, because the champion's
/// construction fills tables a decoder builds once per frame, not per byte.
///
/// # Panics
///
/// Panics if a walk lands a byte other than the one that resolved it, so no
/// time is reported for a walk that did not walk the stream.
fn walk_seconds<M: RateWalk>(streams: &[Vec<(Context, u8)>]) -> f64 {
    let mut seconds = 0.0;
    for stream in streams {
        let mut best = f64::MAX;
        for _ in 0..speed::REPEATS {
            let mut model = M::fresh();
            let mut landed = true;
            let start = Instant::now();
            for &(context, byte) in black_box(stream) {
                landed &= model.walk_byte(context, byte) == byte;
            }
            best = best.min(start.elapsed().as_secs_f64());
            assert!(landed, "a timed walk must land every literal it resolves");
        }
        seconds += best;
    }
    seconds
}

/// Rounds [`rate_vs_champion`] alternates. Odd, so each median is a reading.
const ROUNDS: NonZeroUsize = NonZeroUsize::new(5).expect("nonzero");

/// The least margin a [`RateReport`] states: how far [`RateReport::ratio`]
/// moved across same-code readings on one CPU model, largest over smallest
/// less one, a spread one run's rounds cannot see. Its largest part, 7.4%
/// on a Xeon 6975P-C, came from a build that moved code layout and nothing
/// either walk runs, while that call's rounds agreed with each other
/// (`research/JOURNAL.md` S2-A161). A try changes layout as that build
/// did, so a fall inside this floor is no reading. It is a lower bound:
/// the largest of two layout perturbations, one per model, and a third may
/// move further.
pub const RATE_NOISE_FLOOR: f64 = 0.08;

/// One round of [`rate_over`]: each walk's summed seconds, the champion's
/// read first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateRound {
    /// [`Literal::walk_logit_sse`]'s seconds.
    pub champion: f64,
    /// [`BitMix::walk`]'s seconds.
    pub candidate: f64,
}

impl RateRound {
    /// The candidate's seconds over the champion's.
    #[must_use]
    pub fn ratio(&self) -> f64 {
        self.candidate / self.champion
    }
}

/// The rate driver's reading: both walks' time over the same literal
/// streams, in alternating rounds, beside one calibration run, on the CPU
/// model it names.
///
/// The keep rule reads [`Self::ratio`], the candidate's time over the
/// champion's in the same round: both walk the same contexts on the same
/// core moments apart, so the clock and the machine's load cancel, where
/// the calibration kernel spans up to a third within one model (issue
/// #1184). The CPU model does not cancel: the two walks stress the core
/// differently, so the ratio moves with the microarchitecture, and two
/// readings compare only on one model (`research/JOURNAL.md` S2-A161).
/// Code layout and memory placement do not cancel either, fixed within a
/// process and drawn afresh by each build and run: [`RATE_NOISE_FLOOR`]
/// prices them.
#[derive(Debug, Clone, PartialEq)]
pub struct RateReport {
    /// [`cpu_model`] where the rounds ran; `None` when unreadable, and an
    /// unread model matches no other.
    pub cpu: Option<String>,
    /// Literal bytes per walk, every stream's summed.
    pub literal_bytes: usize,
    /// [`speed::calibration_seconds`], read once before the rounds.
    pub calibration_seconds: f64,
    /// At least one round, in the order they ran.
    pub rounds: Vec<RateRound>,
}

impl RateReport {
    /// Every round's [`RateRound::ratio`], ascending.
    fn sorted_ratios(&self) -> Vec<f64> {
        let mut ratios: Vec<f64> = self.rounds.iter().map(RateRound::ratio).collect();
        ratios.sort_by(f64::total_cmp);
        ratios
    }

    /// The median round's candidate-over-champion time: below 1, the
    /// candidate walks faster.
    #[must_use]
    pub fn ratio(&self) -> f64 {
        median_of_sorted(&self.sorted_ratios())
    }

    /// The spread of the rounds' ratios, largest over smallest less one,
    /// [`speed::CpuSpread::range`]'s convention: how far this run's own
    /// readings disagree.
    #[must_use]
    pub fn spread(&self) -> f64 {
        let ratios = self.sorted_ratios();
        ratios[ratios.len() - 1] / ratios[0] - 1.0
    }

    /// The margin this reading states: its [`Self::spread`], never under
    /// [`RATE_NOISE_FLOOR`], which no single run can see.
    #[must_use]
    pub fn margin(&self) -> f64 {
        self.spread().max(RATE_NOISE_FLOOR)
    }

    /// Whether this reading's ratio falls below `before`'s by more than
    /// the larger of the two margins: a rate try's decode-cost reading,
    /// `before` read on the champion's build and `self` on the try's.
    /// `None` unless both name the same CPU model, because a ratio moves
    /// between models by more than any margin (`research/JOURNAL.md`
    /// S2-A161), so a cross-model verdict would be the CPU's, not the
    /// try's.
    #[must_use]
    pub fn falls_past_margin(&self, before: &Self) -> Option<bool> {
        let same_cpu = self.cpu.is_some() && self.cpu == before.cpu;
        let margin = self.margin().max(before.margin());
        same_cpu.then(|| self.ratio() < before.ratio() * (1.0 - margin))
    }

    /// A walk's median seconds in [`Kernel::Walk`] steps per literal byte,
    /// the unit the charter prices decode cost in (`research/JOURNAL.md`
    /// S2-A141).
    fn cost(&self, walk: fn(&RateRound) -> f64) -> f64 {
        let mut seconds: Vec<f64> = self.rounds.iter().map(walk).collect();
        seconds.sort_by(f64::total_cmp);
        speed::decode_cost(
            Kernel::Walk,
            median_of_sorted(&seconds),
            self.literal_bytes,
            self.calibration_seconds,
        )
    }

    /// [`Literal::walk_logit_sse`]'s median cost, calibration steps per
    /// literal byte.
    #[must_use]
    pub fn champion_cost(&self) -> f64 {
        self.cost(|round| round.champion)
    }

    /// [`BitMix::walk`]'s median cost, calibration steps per literal byte.
    #[must_use]
    pub fn candidate_cost(&self) -> f64 {
        self.cost(|round| round.candidate)
    }

    /// This reading as text [`Self::from_record`] reads back to an equal
    /// report, so a champion's reading taken in one build is a file the
    /// try's build compares against: `bit_mix rate record`, then one line per
    /// field in a fixed order, the CPU line absent when the model was
    /// unread. Seconds print in `f64`'s shortest round-trip form, so no
    /// digit is lost. A CPU model holds no line break, because
    /// [`cpu_model`] reads one trimmed line.
    #[must_use]
    pub fn record(&self) -> String {
        let mut text = format!("{RECORD_HEADER}\n");
        let mut line = |args: fmt::Arguments<'_>| {
            writeln!(text, "{args}").expect("writing to a String never fails");
        };
        if let Some(cpu) = &self.cpu {
            line(format_args!("cpu {cpu}"));
        }
        line(format_args!("literal_bytes {}", self.literal_bytes));
        line(format_args!(
            "calibration_seconds {}",
            self.calibration_seconds
        ));
        for round in &self.rounds {
            line(format_args!("round {} {}", round.champion, round.candidate));
        }
        text
    }

    /// The report [`Self::record`] wrote as `text`.
    ///
    /// # Errors
    ///
    /// Names the first line out of the record's order or format, a time
    /// that is not finite and above zero, because a ratio of it is no
    /// reading, and a record of no round, which has no median.
    pub fn from_record(text: &str) -> Result<Self, String> {
        let lines: Vec<&str> = text.lines().collect();
        if lines.first() != Some(&RECORD_HEADER) {
            return Err(format!("line 1: expected `{RECORD_HEADER}`"));
        }
        let mut at = 1;
        let cpu = lines
            .get(at)
            .and_then(|line| line.strip_prefix("cpu "))
            .map(|cpu| {
                at += 1;
                cpu.to_string()
            });
        let bytes = record_field(&lines, &mut at, "literal_bytes")?;
        let literal_bytes = bytes
            .parse()
            .map_err(|err| format!("line {at}: literal_bytes `{bytes}`: {err}"))?;
        let seconds = record_field(&lines, &mut at, "calibration_seconds")?;
        let calibration_seconds = seconds_of(at, seconds)?;
        let mut rounds = Vec::new();
        while at < lines.len() {
            let pair = record_field(&lines, &mut at, "round")?;
            let (champion, candidate) = pair
                .split_once(' ')
                .ok_or_else(|| format!("line {at}: expected `round <champion> <candidate>`"))?;
            rounds.push(RateRound {
                champion: seconds_of(at, champion)?,
                candidate: seconds_of(at, candidate)?,
            });
        }
        if rounds.is_empty() {
            return Err("holds no `round` line".to_string());
        }
        Ok(Self {
            cpu,
            literal_bytes,
            calibration_seconds,
            rounds,
        })
    }
}

/// [`RateReport::record`]'s first line, which names the file a reading.
const RECORD_HEADER: &str = "bit_mix rate record";

/// The value of `lines[*at]`, which must read `<key> <value>`, advancing
/// `at` past it, so an error's `line {at}` is the line it names.
fn record_field<'a>(lines: &[&'a str], at: &mut usize, key: &str) -> Result<&'a str, String> {
    let line = lines
        .get(*at)
        .ok_or_else(|| format!("ends before its `{key}` line"))?;
    *at += 1;
    line.strip_prefix(key)
        .and_then(|rest| rest.strip_prefix(' '))
        .ok_or_else(|| format!("line {at}: expected `{key} <value>`, found `{line}`"))
}

/// `value`, from record line `line`, as seconds a ratio can divide by.
fn seconds_of(line: usize, value: &str) -> Result<f64, String> {
    match value.parse::<f64>() {
        Ok(seconds) if seconds.is_finite() && seconds > 0.0 => Ok(seconds),
        _ => Err(format!(
            "line {line}: `{value}` is no finite time above zero"
        )),
    }
}

impl fmt::Display for RateReport {
    #[allow(
        clippy::cast_precision_loss,
        reason = "a literal byte count stays far below 2^53"
    )]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.literal_bytes as f64;
        writeln!(
            f,
            "bit_mix rate on {}: {} literal bytes, ns per literal byte",
            self.cpu.as_deref().unwrap_or("unknown CPU"),
            self.literal_bytes
        )?;
        for round in &self.rounds {
            writeln!(
                f,
                "  champion {:.1}  candidate {:.1}  ratio {:.4}",
                round.champion / bytes * 1e9,
                round.candidate / bytes * 1e9,
                round.ratio()
            )?;
        }
        write!(
            f,
            "ratio {:.4}, spread {:.1}%, margin {:.1}%; calibration steps per literal byte: champion {:.2}, candidate {:.2}",
            self.ratio(),
            self.spread() * 100.0,
            self.margin() * 100.0,
            self.champion_cost(),
            self.candidate_cost()
        )
    }
}

/// [`RateReport`] over `streams`: one calibration, then `rounds` rounds,
/// each timing the champion's walk and then the candidate's.
#[must_use]
pub fn rate_over(streams: &[Vec<(Context, u8)>], rounds: NonZeroUsize) -> RateReport {
    let calibration_seconds = speed::calibration_seconds();
    let rounds = (0..rounds.get())
        .map(|_| RateRound {
            champion: walk_seconds::<ChampionWalk>(streams),
            candidate: walk_seconds::<BitMix>(streams),
        })
        .collect();
    RateReport {
        cpu: cpu_model(),
        literal_bytes: streams.iter().map(Vec::len).sum(),
        calibration_seconds,
        rounds,
    }
}

/// The class's rate driver: [`rate_over`] the eleven train cases' literal
/// streams in five rounds. Read it in a release build, once on the
/// champion's build and once on the try's, on one CPU model, and keep the
/// try when [`RateReport::falls_past_margin`] answers `Some(true)`. The
/// `bit_mix_rate` binary is that procedure: `measure > <file>` on the
/// champion's build, `verdict <file>` on the try's.
#[must_use]
pub fn rate_vs_champion() -> RateReport {
    let streams: Vec<_> = baseline::cases()
        .iter()
        .map(|case| literal_stream(&case.data))
        .collect();
    rate_over(&streams, ROUNDS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literal_pairing::case_delta_bpb;

    /// A text whose parse leaves literals: words recombined, so the
    /// parse copies some and codes the rest.
    fn text() -> Vec<u8> {
        let words = ["mother", "god", "literal", "coder", "class", "bit", "mix"];
        (0..2_000)
            .flat_map(|i: usize| {
                let word = words[(i * 7 + i / 3) % words.len()];
                word.bytes().chain(std::iter::once(b' '))
            })
            .collect()
    }

    #[test]
    fn each_pricer_charges_every_literal_a_positive_cost_under_twelve_bits_a_node() {
        let data = text();
        for (name, bits) in [
            ("champion", literal_replay(&data, champion())),
            ("candidate", literal_replay(&data, candidate())),
        ] {
            let (literals, total) = bits;
            assert!(!literals.is_empty(), "{name}: the parse left no literal");
            #[allow(clippy::cast_precision_loss, reason = "a test buffer's literal count")]
            let ceiling = 8.0 * 12.0 * literals.len() as f64;
            assert!(
                total > 0.0 && total < ceiling,
                "{name}: {total} bits for {} literals",
                literals.len()
            );
        }
    }

    #[test]
    fn the_candidate_prices_differently_from_the_champion() {
        let delta = case_delta_bpb(&text(), champion(), candidate());
        assert!(delta.is_finite() && delta != 0.0, "delta {delta}");
    }

    /// The recorded stream is the replay's: its bytes are the parse's
    /// literals, and the champion priced over its contexts from a fresh
    /// model charges exactly what [`literal_replay`] charges.
    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "the same pricer over the same contexts from the same fresh state sums the same floats in the same order"
    )]
    fn a_literal_stream_is_the_replay_s_contexts_and_bytes_in_coding_order() {
        let data = text();
        let (literals, replay_bits) = literal_replay(&data, champion());

        let stream = literal_stream(&data);

        let bytes: Vec<u8> = stream.iter().map(|&(_, byte)| byte).collect();
        assert_eq!(bytes, literals);
        let mut model = Literal::new();
        let mut price = champion();
        let stream_bits: f64 = stream
            .iter()
            .map(|&(context, byte)| price(&mut model, context, byte))
            .sum();
        assert_eq!(stream_bits, replay_bits);
    }

    /// A model that lands every byte one above the byte that resolved it.
    struct OffByOne;

    impl RateWalk for OffByOne {
        fn fresh() -> Self {
            Self
        }

        fn walk_byte(&mut self, _: Context, byte: u8) -> u8 {
            byte.wrapping_add(1)
        }
    }

    #[test]
    fn a_resolved_bit_depends_on_the_probability_and_splits_at_the_midpoint_for_a_real_one() {
        for byte in [0u8, 127, 128, 255] {
            for mid in [1usize, 128, 255] {
                assert_eq!(resolve(byte, mid, 0.5), usize::from(byte) >= mid);
                assert_ne!(resolve(byte, mid, f64::NAN), resolve(byte, mid, 0.5));
            }
        }
    }

    #[test]
    #[should_panic(expected = "a timed walk must land every literal it resolves")]
    fn a_walk_that_lands_another_byte_reports_no_time() {
        let _ = walk_seconds::<OffByOne>(&[literal_stream(&text())]);
    }

    #[test]
    fn both_walks_land_every_literal_and_a_report_holds_every_round() {
        let streams = [literal_stream(&text()), literal_stream(b"mother god")];
        let rounds = NonZeroUsize::new(2).expect("nonzero");

        let report = rate_over(&streams, rounds);

        assert_eq!(report.literal_bytes, streams[0].len() + streams[1].len());
        assert_eq!(report.rounds.len(), 2);
        assert!(report.calibration_seconds > 0.0);
        for round in &report.rounds {
            assert!(round.champion > 0.0 && round.candidate > 0.0, "{round:?}");
        }
        assert!(report.ratio().is_finite() && report.ratio() > 0.0);
    }

    /// A report on one named CPU model, of 1,000 literal bytes, a
    /// calibration of one microsecond a step, and rounds of a champion at
    /// one second against `candidates`.
    fn report(candidates: &[f64]) -> RateReport {
        RateReport {
            cpu: Some("Xeon 6975P-C".to_string()),
            literal_bytes: 1_000,
            calibration_seconds: 4.0,
            rounds: candidates
                .iter()
                .map(|&candidate| RateRound {
                    champion: 1.0,
                    candidate,
                })
                .collect(),
        }
    }

    #[test]
    fn the_ratio_is_the_median_round_and_the_margin_its_spread_above_the_floor() {
        let reading = report(&[0.31, 0.29, 0.30]);
        assert!((reading.ratio() - 0.30).abs() < 1e-12);
        let spread = 0.31 / 0.29 - 1.0;
        assert!((reading.spread() - spread).abs() < 1e-12);
        assert!(spread < RATE_NOISE_FLOOR);
        assert!((reading.margin() - RATE_NOISE_FLOOR).abs() < 1e-12);
        let even = report(&[0.2, 0.4]);
        assert!((even.ratio() - 0.3).abs() < 1e-12, "{}", even.ratio());
        assert!((even.spread() - 1.0).abs() < 1e-12, "{}", even.spread());
        assert!((even.margin() - 1.0).abs() < 1e-12, "{}", even.margin());
    }

    #[test]
    fn a_try_is_faster_only_past_the_wider_of_both_margins() {
        let before = report(&[0.31, 0.29, 0.30]);
        assert_eq!(
            report(&[0.25, 0.26, 0.255]).falls_past_margin(&before),
            Some(true)
        );
        assert_eq!(
            report(&[0.29, 0.29, 0.29]).falls_past_margin(&before),
            Some(false),
            "faster, but inside before's margin"
        );
        assert_eq!(
            report(&[0.35, 0.35, 0.35]).falls_past_margin(&before),
            Some(false)
        );
        let steady = report(&[0.30, 0.30, 0.30]);
        assert_eq!(
            report(&[0.25, 0.25, 0.25]).falls_past_margin(&steady),
            Some(true)
        );
        assert_eq!(
            report(&[0.20, 0.25, 0.28]).falls_past_margin(&steady),
            Some(false),
            "faster, but inside the try's own margin"
        );
    }

    #[test]
    fn a_fall_inside_the_noise_floor_is_kept_by_neither_spread() {
        let steady = report(&[0.30, 0.30, 0.30]);
        assert_eq!(
            report(&[0.28, 0.28, 0.28]).falls_past_margin(&steady),
            Some(false),
            "6.7% faster with no spread on either side"
        );
        assert_eq!(
            report(&[0.27, 0.27, 0.27]).falls_past_margin(&steady),
            Some(true)
        );
    }

    /// The reviewer's EPYC 7763 read about 21% above the Xeon 6975P-C on
    /// the same code (`research/JOURNAL.md` S2-A161), so a fall that
    /// clears every margin is still no verdict across models.
    #[test]
    fn readings_on_two_cpu_models_or_an_unread_one_give_no_verdict() {
        let before = report(&[0.30, 0.30, 0.30]);
        let mut other_model = report(&[0.20, 0.20, 0.20]);
        other_model.cpu = Some("AMD EPYC 7763 64-Core Processor".to_string());
        assert_eq!(other_model.falls_past_margin(&before), None);
        assert_eq!(before.falls_past_margin(&other_model), None);

        let mut unread = report(&[0.20, 0.20, 0.20]);
        unread.cpu = None;
        let mut unread_before = before.clone();
        unread_before.cpu = None;
        assert_eq!(unread.falls_past_margin(&unread_before), None);
        assert_eq!(unread.falls_past_margin(&before), None);
    }

    #[test]
    fn a_cost_is_calibration_steps_per_literal_byte_at_the_median_round() {
        // One second over 1,000 bytes is 1 ms a byte; a step is 1 us.
        let reading = report(&[0.5, 0.25, 0.75]);
        assert!((reading.champion_cost() - 1_000.0).abs() < 1e-9);
        assert!((reading.candidate_cost() - 500.0).abs() < 1e-9);
    }

    #[test]
    fn a_record_reads_back_to_the_report_it_was_written_from() {
        // Seconds with no short decimal form, so a lossy print would show.
        let mut reading = report(&[0.1 + 0.2, 1.0 / 3.0, 2e-9]);
        reading.calibration_seconds = std::f64::consts::PI;
        assert_eq!(
            RateReport::from_record(&reading.record()),
            Ok(reading.clone())
        );

        reading.cpu = None;
        let record = reading.record();
        assert!(!record.contains("cpu"), "{record}");
        assert_eq!(RateReport::from_record(&record), Ok(reading));
    }

    #[test]
    fn a_record_prints_one_line_per_field_in_order() {
        assert_eq!(
            report(&[0.5, 0.25]).record(),
            "bit_mix rate record\ncpu Xeon 6975P-C\nliteral_bytes 1000\n\
             calibration_seconds 4\nround 1 0.5\nround 1 0.25\n"
        );
    }

    /// Every way a record can fail names the line, so a hand-edited or
    /// truncated champion file is refused rather than read as a reading.
    #[test]
    fn a_record_out_of_order_or_format_or_without_a_round_is_refused() {
        let good = report(&[0.5, 0.25]).record();
        for (text, error) in [
            (String::new(), "line 1: expected `bit_mix rate record`"),
            (
                good.replace("bit_mix rate record", "{\"cases\": {}}"),
                "line 1: expected `bit_mix rate record`",
            ),
            (
                "bit_mix rate record\ncpu Xeon 6975P-C\n".to_string(),
                "ends before its `literal_bytes` line",
            ),
            (
                good.replace("literal_bytes 1000", "literal_bytes -1"),
                "line 3: literal_bytes `-1`: invalid digit found in string",
            ),
            (
                good.replace("round 1 0.5\nround 1 0.25\n", ""),
                "holds no `round` line",
            ),
            (
                good.replace("round 1 0.25", "round 1"),
                "line 6: expected `round <champion> <candidate>`",
            ),
            (
                format!("{good}ratio 0.5\n"),
                "line 7: expected `round <value>`, found `ratio 0.5`",
            ),
            (
                good.replace("calibration_seconds 4", "calibration_seconds 0"),
                "line 4: `0` is no finite time above zero",
            ),
            (
                good.replace("round 1 0.5", "round NaN 0.5"),
                "line 5: `NaN` is no finite time above zero",
            ),
            (
                good.replace("round 1 0.25", "round 1 inf"),
                "line 6: `inf` is no finite time above zero",
            ),
        ] {
            assert_eq!(
                RateReport::from_record(&text),
                Err(error.to_string()),
                "{text}"
            );
        }
    }

    #[test]
    fn a_report_prints_every_round_then_the_reading() {
        assert_eq!(
            report(&[0.5, 0.25]).to_string(),
            "bit_mix rate on Xeon 6975P-C: 1000 literal bytes, ns per literal byte\n  \
             champion 1000000.0  candidate 500000.0  ratio 0.5000\n  \
             champion 1000000.0  candidate 250000.0  ratio 0.2500\n\
             ratio 0.3750, spread 100.0%, margin 100.0%; calibration steps per literal byte: \
             champion 1000.00, candidate 375.00"
        );
    }
}
