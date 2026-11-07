//! The decode stage table of issue #1930: where the default level's decode
//! second goes, per stage, from `perf` samples of a release build with line
//! tables (journal S2-R43, charter #1922 question 1).
//!
//! Pure over the text `perf script -F ip,sym,dso,period --inline` prints
//! for a `perf record -g` run: no process, no file, so the parser and the
//! classifier build and test under the default features. The
//! `decode_profile` binary runs the processes and prints [`format_table`].
//!
//! A sample is attributed by its leaf's inline chain, the frames `perf`
//! expands at the sampled instruction, innermost first: the first frame a
//! [`RULES`] entry matches decides the stage. Inlining is why the chain and
//! not the symbol: `expert_prefix_sums` and the walk both compile into
//! `decode_logit_sse`, and `ln` runs inside both `logistic_mix_node` and
//! `locate`, so the symbol alone cannot tell them apart. Callers beyond the
//! leaf are not read: a release build omits frame pointers, so `-g`'s chain
//! past the leaf is unreliable, and an out-of-line `floor` called by
//! `locate` counts as [`Stage::Libm`], not as SSE (S2-R43 split those by
//! hand).

use std::fmt::Write as _;

/// One row of the stage table, in the order the table prints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The literal model's SSE lookup.
    SseLocate,
    /// The literal model's six-expert logistic mix per node.
    Mix,
    /// The per-byte cumulative tables over each expert's bank.
    PrefixSums,
    /// The bit-tree walk: squash, SSE refine and update, gradients.
    Walk,
    /// The range decoder's literal bit.
    BitDecode,
    /// The literal model's remaining work: `exp`, `pow2`, bank update.
    LiteralRest,
    /// The float functions of `compiler_builtins`, inline or called.
    Libm,
    /// Token flags, lengths, slots and buckets, and the token loop.
    TokenModel,
    /// Raw offset bits.
    OffsetBits,
    /// The match copy and the literal context's catch-up after it.
    Copy,
    /// The C library: `memcpy` and the output writes.
    Libc,
    /// Kernel time: syscalls and page faults.
    Kernel,
    /// Everything no rule names: inverse filter, frames, checksum.
    Rest,
}

impl Stage {
    /// Every stage, in the order the table prints.
    pub const ALL: [Stage; 13] = [
        Stage::SseLocate,
        Stage::Mix,
        Stage::PrefixSums,
        Stage::Walk,
        Stage::BitDecode,
        Stage::LiteralRest,
        Stage::Libm,
        Stage::TokenModel,
        Stage::OffsetBits,
        Stage::Copy,
        Stage::Libc,
        Stage::Kernel,
        Stage::Rest,
    ];

    /// The part of the decoder the stage belongs to, the label's prefix.
    #[must_use]
    pub const fn area(self) -> &'static str {
        match self {
            Stage::SseLocate
            | Stage::Mix
            | Stage::PrefixSums
            | Stage::Walk
            | Stage::BitDecode
            | Stage::LiteralRest => "literal",
            Stage::Libm => "libm",
            Stage::TokenModel | Stage::OffsetBits => "token",
            Stage::Copy => "copy",
            Stage::Libc => "libc",
            Stage::Kernel => "kernel",
            Stage::Rest => "rest",
        }
    }

    /// The table's row label: the area and the functions [`RULES`] maps to
    /// the stage, so a label never names a function the rules do not.
    #[must_use]
    pub fn label(self) -> String {
        let functions: Vec<String> = RULES
            .iter()
            .filter(|(_, stage)| *stage == self)
            .map(|(path, _)| format!("`{}`", path.join("::")))
            .collect();
        if functions.is_empty() {
            self.area().to_string()
        } else {
            format!("{}: {}", self.area(), functions.join(", "))
        }
    }

    const fn index(self) -> usize {
        self as usize
    }
}

/// Function paths and the stage each one decides, matched against a
/// frame's path suffix (see [`segments`]). Every path but a
/// [`Stage::Libm`] one names a function in `src/`, one segment where the
/// name is defined once, two where it is not (`Model::decode`); a test
/// holds that, so a renamed function fails CI instead of moving its time
/// to [`Stage::Rest`].
pub const RULES: &[(&[&str], Stage)] = &[
    (&["locate"], Stage::SseLocate),
    (&["logistic_mix_node"], Stage::Mix),
    (&["expert_prefix_sums"], Stage::PrefixSums),
    (&["walk_nodes"], Stage::Walk),
    (&["walk_logit_sse"], Stage::Walk),
    (&["decode_bit"], Stage::BitDecode),
    (&["exp"], Stage::LiteralRest),
    (&["pow2"], Stage::LiteralRest),
    (&["expert_estimates"], Stage::LiteralRest),
    (&["Literal", "update"], Stage::LiteralRest),
    (&["decode_logit_sse"], Stage::LiteralRest),
    (&["floor"], Stage::Libm),
    (&["round"], Stage::Libm),
    (&["fma"], Stage::Libm),
    (&["Model", "decode"], Stage::TokenModel),
    (&["decode_tokens"], Stage::TokenModel),
    (&["decode_bits"], Stage::OffsetBits),
    (&["decode_offset"], Stage::OffsetBits),
    (&["StreamingSink", "copy"], Stage::Copy),
    (&["after_copy"], Stage::Copy),
];

/// Lowest address of the x86-64 kernel half: a leaf at or above it is
/// kernel code, which `perf` cannot name under `kptr_restrict`.
const KERNEL_START: u64 = 0xffff_8000_0000_0000;

/// `name` with generic arguments removed and qualified paths unwrapped,
/// split into its path segments: `<a::Table<a::Logit>>::locate` is
/// `[a, Table, locate]`, `<a::Sink<T> as a::DecodeSink>::copy` is
/// `[a, Sink, copy]`, and `update<a::Logit>` is `[update]`.
///
/// A `<` after an identifier, `}` or `::` opens generic arguments, skipped
/// to their matching `>`; any other `<` opens a qualified path, whose
/// `as <trait>` part is skipped.
#[must_use]
pub fn segments(name: &str) -> Vec<String> {
    let mut path = String::with_capacity(name.len());
    // Depth inside skipped generic arguments.
    let mut generic = 0usize;
    // Open qualified-path brackets, and whether their `as <trait>` part is
    // being skipped.
    let mut qualified = 0usize;
    let mut trait_part = false;
    let mut prev = None;
    for c in name.chars() {
        if generic > 0 {
            match c {
                '<' => generic += 1,
                // `->` inside a fn-pointer argument closes nothing.
                '>' if prev != Some('-') => generic -= 1,
                _ => {}
            }
        } else if c == '<' {
            if prev.is_some_and(|p: char| p.is_alphanumeric() || matches!(p, '_' | '}' | ':')) {
                generic = 1;
            } else {
                qualified += 1;
            }
        } else if c == '>' && qualified > 0 {
            qualified -= 1;
            trait_part = false;
        } else if !trait_part {
            path.push(c);
            if let Some(stripped) = path.strip_suffix(" as ") {
                path.truncate(stripped.len());
                trait_part = true;
            }
        }
        prev = Some(c);
    }
    path.split("::")
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect()
}

/// The stage the first [`RULES`] path decides for a frame named `name`,
/// or `None` where no rule matches it.
#[must_use]
pub fn rule_stage(name: &str) -> Option<Stage> {
    let path = segments(name);
    RULES
        .iter()
        .find(|(rule, _)| {
            path.len() >= rule.len()
                && path
                    .iter()
                    .rev()
                    .zip(rule.iter().rev())
                    .all(|(a, b)| a == b)
        })
        .map(|(_, stage)| *stage)
}

/// One frame line of `perf script` output.
#[derive(Debug, PartialEq, Eq)]
struct Frame<'a> {
    ip: u64,
    name: &'a str,
    /// The object the frame's code lives in; `None` on an inlined frame,
    /// whose object is the next non-inlined frame's.
    dso: Option<&'a str>,
}

/// Parses `<ip> <name> (inlined)` or `<ip> <name> (<dso>)`.
fn parse_frame(line: &str) -> Option<Frame<'_>> {
    let (ip, rest) = line.trim().split_once(' ')?;
    let ip = u64::from_str_radix(ip, 16).ok()?;
    if let Some(name) = rest.strip_suffix(" (inlined)") {
        return Some(Frame {
            ip,
            name,
            dso: None,
        });
    }
    let (name, dso) = rest.strip_suffix(')')?.rsplit_once(" (")?;
    Some(Frame {
        ip,
        name,
        dso: Some(dso),
    })
}

/// The stage of one leaf frame, before the rules: kernel code by address,
/// the C library by its object.
fn object_stage(frame: &Frame<'_>) -> Option<Stage> {
    if frame.ip >= KERNEL_START {
        return Some(Stage::Kernel);
    }
    let object = frame.dso?.rsplit('/').next()?;
    object.starts_with("libc.so").then_some(Stage::Libc)
}

/// One sample of `perf script` output while its lines are read.
struct Block {
    /// The sample's period.
    period: u64,
    /// The sampled address, the first frame's; every frame of the leaf's
    /// inline chain carries it.
    leaf: Option<u64>,
    /// The stage the first matching frame decided.
    stage: Option<Stage>,
    /// Whether a frame of another address, a caller, has been read.
    past_leaf: bool,
}

/// Sampled time per [`Stage`], summed over any number of `perf script`
/// outputs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// Sample periods per stage, indexed by [`Stage::ALL`]'s order; a
    /// `task-clock` period is nanoseconds of CPU time.
    pub periods: [u64; Stage::ALL.len()],
    /// Samples read.
    pub samples: u64,
}

impl Tally {
    /// Adds every sample of `script`, the output of `perf script -F
    /// ip,sym,dso,period --inline` on a `perf record -g` run: blocks
    /// separated by blank lines, each a period line and then frame lines,
    /// the leaf's inline chain first.
    ///
    /// # Errors
    ///
    /// The 1-based line number and text of the first line that is neither
    /// a period where a block starts nor a frame after it, so a change in
    /// `perf`'s format fails loudly instead of counting nothing.
    pub fn add(&mut self, script: &str) -> Result<(), String> {
        let mut block: Option<Block> = None;
        for (number, line) in script.lines().enumerate() {
            let bad = || format!("perf script line {}: {line:?}", number + 1);
            if line.trim().is_empty() {
                self.close(block.take());
                continue;
            }
            let Some(open) = block.as_mut() else {
                let period = line.trim().parse().map_err(|_| bad())?;
                block = Some(Block {
                    period,
                    leaf: None,
                    stage: None,
                    past_leaf: false,
                });
                continue;
            };
            let frame = parse_frame(line).ok_or_else(bad)?;
            if *open.leaf.get_or_insert(frame.ip) != frame.ip {
                open.past_leaf = true;
            }
            if open.past_leaf || open.stage.is_some() {
                continue;
            }
            open.stage = rule_stage(frame.name).or_else(|| object_stage(&frame));
        }
        self.close(block);
        Ok(())
    }

    fn close(&mut self, block: Option<Block>) {
        if let Some(block) = block {
            self.periods[block.stage.unwrap_or(Stage::Rest).index()] += block.period;
            self.samples += 1;
        }
    }

    /// Every stage's period, summed.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.periods.iter().sum()
    }
}

/// Nanoseconds as seconds.
#[allow(
    clippy::cast_precision_loss,
    reason = "a profile's nanoseconds stay far below 2^53"
)]
fn seconds(nanos: u64) -> f64 {
    nanos as f64 / 1e9
}

/// `tally` as a markdown table: one row per [`Stage`] in [`Stage::ALL`]'s
/// order, every row printed even at zero so two runs line up, then the
/// total. Share is of the sampled time; seconds are CPU seconds, a
/// `task-clock` period being nanoseconds.
#[must_use]
pub fn format_table(tally: &Tally) -> String {
    let total = tally.total();
    let mut out = String::from("| stage | share | s |\n|---|---|---|\n");
    for stage in Stage::ALL {
        let period = tally.periods[stage.index()];
        let share = if total == 0 {
            0.0
        } else {
            seconds(period) / seconds(total) * 100.0
        };
        writeln!(
            out,
            "| {} | {share:.1}% | {:.2} |",
            stage.label(),
            seconds(period)
        )
        .expect("writing to a String never fails");
    }
    writeln!(out, "| total | 100.0% | {:.2} |", seconds(total))
        .expect("writing to a String never fails");
    out
}

#[cfg(test)]
mod tests {
    use super::{RULES, Stage, Tally, format_table, rule_stage, segments};

    /// Five samples as `perf script -F ip,sym,dso,period --inline` printed
    /// them for a `perf record -g` decode (perf 7.0.14), binary path cut.
    /// The second's leaf is in the kernel under a libc caller; the third's
    /// chain resolves through `ln` and `stretch` to `locate`.
    const SCRIPT: &str = "   1001001 \n\
\t    5a2aff2aa5e9 expert_prefix_sums (inlined)\n\
\t    5a2aff2aa5e9 walk_logit_sse<mothergod::literal::{impl#5}::decode_logit_sse::{closure_env#0}> (inlined)\n\
\t    5a2aff2aa5e9 <mothergod::literal::Literal>::decode_logit_sse (mothergod)\n\
\t    74bc05946010 [unknown] ([unknown])\n\
\n\
\n\
   2000000 \n\
\tffffffff90422188 [unknown] ([unknown])\n\
\tffffffff91771443 [unknown] ([unknown])\n\
\t    74bc065a33e6 [unknown] (/usr/lib/x86_64-linux-gnu/libc.so.6)\n\
\n\
   1001001 \n\
\t    5a2aff2b6aff ln (inlined)\n\
\t    5a2aff2b6aff stretch (inlined)\n\
\t    5a2aff2b6aff position (inlined)\n\
\t    5a2aff2b6aff <mothergod::sse::Table<mothergod::sse::Logit>>::locate (mothergod)\n\
\n\
   1001001 \n\
\t    74bc065a33b7 [unknown] (/usr/lib/x86_64-linux-gnu/libc.so.6)\n\
\n\
   1001001 \n\
\t    5a2aff29cf30 {closure#0} (inlined)\n\
\t    5a2aff29cf30 fold<u8, u32, mothergod::crc32::crc32::{closure_env#0}> (inlined)\n\
\t    5a2aff29cf30 crc32 (inlined)\n\
\t    5a2aff29cf30 <mothergod::Frames as core::iter::traits::iterator::Iterator>::next (mothergod)\n\
\t    5a2aff29d000 <mothergod::literal::Literal>::decode_logit_sse (mothergod)\n";

    fn period(tally: &Tally, stage: Stage) -> u64 {
        tally.periods[Stage::ALL.iter().position(|s| *s == stage).expect("listed")]
    }

    #[test]
    fn all_lists_every_stage_once_in_index_order() {
        for (i, stage) in Stage::ALL.into_iter().enumerate() {
            assert_eq!(stage.index(), i, "{stage:?}");
        }
    }

    #[test]
    fn segments_drop_generics_and_unwrap_qualified_paths() {
        for (name, want) in [
            (
                "<mothergod::sse::Table<mothergod::sse::Logit>>::locate",
                &["mothergod", "sse", "Table", "locate"][..],
            ),
            (
                "<mothergod::codec::StreamingSink<mothergod::TryBufWriter<mothergod::Counted<&mut dyn core::io::write::Write>>> as mothergod::codec::DecodeSink>::copy",
                &["mothergod", "codec", "StreamingSink", "copy"],
            ),
            ("update<mothergod::sse::Logit>", &["update"]),
            (
                "mothergod::bittree::walk_nodes::<<mothergod::literal::Literal>::walk_logit_sse<<mothergod::literal::Literal>::decode_logit_sse::{closure#0}>::{closure#0}>",
                &["mothergod", "bittree", "walk_nodes"],
            ),
            (
                "{closure#0}<mothergod::literal::{impl#5}::decode_logit_sse::{closure_env#0}>",
                &["{closure#0}"],
            ),
            ("apply<fn(u8) -> u8>", &["apply"]),
            ("[unknown]", &["[unknown]"]),
        ] {
            assert_eq!(segments(name), want, "{name}");
        }
    }

    #[test]
    fn a_rule_matches_a_path_suffix_and_never_a_part_of_a_segment() {
        assert_eq!(
            rule_stage("<mothergod::model::Model>::decode"),
            Some(Stage::TokenModel)
        );
        // The coder's inlined `decode` is not `Model::decode`.
        assert_eq!(rule_stage("decode"), None);
        // An SSE `update` is not `Literal::update`.
        assert_eq!(rule_stage("update<mothergod::sse::Logit>"), None);
        assert_eq!(
            rule_stage("<mothergod::literal::Literal>::update"),
            Some(Stage::LiteralRest)
        );
        assert_eq!(rule_stage("exp_unbiased"), None);
        assert_eq!(
            rule_stage("compiler_builtins::math::libm_math::floor::floor"),
            Some(Stage::Libm)
        );
    }

    #[test]
    fn a_sample_takes_the_first_stage_its_leaf_chain_names() {
        let mut tally = Tally::default();
        tally.add(SCRIPT).expect("well-formed");
        assert_eq!(tally.samples, 5);
        assert_eq!(period(&tally, Stage::PrefixSums), 1_001_001);
        assert_eq!(period(&tally, Stage::Kernel), 2_000_000);
        assert_eq!(period(&tally, Stage::SseLocate), 1_001_001);
        assert_eq!(period(&tally, Stage::Libc), 1_001_001);
        // The checksum's chain names no rule, and its caller is past the
        // leaf, so `decode_logit_sse` does not claim it.
        assert_eq!(period(&tally, Stage::Rest), 1_001_001);
        assert_eq!(tally.total(), 6_004_004);
    }

    #[test]
    fn tallies_add_across_outputs() {
        let mut tally = Tally::default();
        tally.add(SCRIPT).expect("well-formed");
        tally.add(SCRIPT).expect("well-formed");
        assert_eq!((tally.samples, tally.total()), (10, 12_008_008));
    }

    #[test]
    fn a_line_outside_the_format_is_an_error_naming_it() {
        let mut tally = Tally::default();
        // A frame where a block's period belongs: `-F` lacked `period`.
        assert_eq!(
            tally.add("\t    5a2aff29cf30 crc32 (inlined)\n"),
            Err("perf script line 1: \"\\t    5a2aff29cf30 crc32 (inlined)\"".to_string())
        );
        let err = tally
            .add("   1001001 \n\tnot-hex crc32 (inlined)\n")
            .expect_err("no address");
        assert!(err.starts_with("perf script line 2: "), "{err}");
        let err = tally
            .add("   1001001 \n\t    5a2aff29cf30 crc32\n")
            .expect_err("neither inlined nor an object");
        assert!(err.starts_with("perf script line 2: "), "{err}");
    }

    #[test]
    fn table_prints_every_stage_in_order_and_the_total() {
        let mut tally = Tally::default();
        tally.add(SCRIPT).expect("well-formed");
        let table = format_table(&tally);
        let rows: Vec<&str> = table.lines().collect();
        assert_eq!(rows.len(), 2 + Stage::ALL.len() + 1, "{table}");
        assert_eq!(rows[2], "| literal: `locate` | 16.7% | 0.00 |", "{table}");
        assert_eq!(rows[4], "| literal: `expert_prefix_sums` | 16.7% | 0.00 |");
        assert_eq!(rows[13], "| kernel | 33.3% | 0.00 |");
        assert_eq!(rows[15], "| total | 100.0% | 0.01 |");
        // A tally of nothing prints zeros, not a division by zero.
        assert!(format_table(&Tally::default()).contains("| rest | 0.0% | 0.00 |"));
    }

    /// Whether `fn <name>` opens a definition in `text`: followed by `(`
    /// or by generic parameters.
    fn defines(text: &str, name: &str) -> usize {
        text.matches(&format!("fn {name}("))
            .chain(text.matches(&format!("fn {name}<")))
            .count()
    }

    #[test]
    fn every_rule_but_libm_names_a_function_in_src() {
        let mut sources = String::new();
        let mut dirs = vec![crate::repo_root().join("src")];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).expect("src/ is readable") {
                let path = entry.expect("a directory entry").path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    sources.push_str(&std::fs::read_to_string(&path).expect("a source file"));
                }
            }
        }
        for (path, stage) in RULES.iter().filter(|(_, stage)| *stage != Stage::Libm) {
            let name = path.last().expect("a rule names a function");
            let found = defines(&sources, name);
            match path {
                // One segment identifies the function only where it is
                // defined once.
                [_] => assert_eq!(found, 1, "{path:?} ({stage:?}) is defined {found} times"),
                [owner, ..] => {
                    assert!(found > 0, "{path:?} ({stage:?}) names no function");
                    assert!(
                        [
                            format!("struct {owner}"),
                            format!("enum {owner}"),
                            format!("trait {owner}")
                        ]
                        .iter()
                        .any(|item| sources.contains(item.as_str())),
                        "{path:?} ({stage:?}) names no type {owner}"
                    );
                }
                [] => unreachable!("a rule names a function"),
            }
        }
    }
}
