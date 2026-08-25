//! Parses `research/progress.jsonl` (schema: `research/README.md`) into
//! [`Experiment`] rows for `site/status.html`'s experiment ledger. Reads
//! only the three fields the page shows (`id`, `date`, `verdict`, plus
//! `hypothesis` for the latest entry's one-liner) with a hand-rolled line
//! scan, not a general JSON parser — the same deliberate scope limit
//! `bench::baseline`'s reader takes (`bench/src/baseline.rs`): this format
//! only ever needs to round-trip what this project itself writes to it.

use crate::json;

/// One `research/progress.jsonl` row, reduced to what the status page
/// shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Experiment {
    /// `"itNN"`.
    pub id: String,
    /// `"YYYY-MM-DD"`.
    pub date: String,
    /// `verdict` field, restricted to the two values the schema allows.
    pub verdict: Verdict,
    /// `hypothesis` field, unescaped plain text.
    pub hypothesis: String,
}

/// A recorded experiment's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Ratio/infra improvement kept.
    Accepted,
    /// Hypothesis falsified; recorded anyway (CLAUDE.md hard rule 6).
    Rejected,
}

/// Extracts a `"key": "<value>"` string field's raw (still JSON-escaped)
/// body from one JSONL line. `None` if `key` is absent, its value isn't a
/// JSON string (e.g. `null` delta fields), or the string is unterminated.
fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\": \"");
    let start = line.find(&needle)? + needle.len();
    let bytes = line.as_bytes();
    let mut i = start;
    let mut escaped = false;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if !escaped => escaped = true,
            b'"' if !escaped => return Some(&line[start..i]),
            _ => escaped = false,
        }
        i += 1;
    }
    None
}

/// Parses every non-empty line of `research/progress.jsonl`'s text into
/// an [`Experiment`], in file order (the log is append-only, so this is
/// chronological — [`crate::experiments`]'s callers rely on the last
/// element being the newest). A line missing `id`/`date`/`verdict`, or
/// whose `verdict` is neither `"accepted"` nor `"rejected"`, is dropped
/// rather than erroring: a reader that panics on its own project's log
/// drifting is a worse failure mode than one that quietly skips a
/// malformed row.
#[must_use]
pub fn parse_progress(jsonl: &str) -> Vec<Experiment> {
    jsonl
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let id = field(line, "id")?;
            let date = field(line, "date")?;
            let verdict = match field(line, "verdict")? {
                "accepted" => Verdict::Accepted,
                "rejected" => Verdict::Rejected,
                _ => return None,
            };
            let hypothesis = field(line, "hypothesis").unwrap_or_default();
            Some(Experiment {
                id: id.to_string(),
                date: date.to_string(),
                verdict,
                hypothesis: json::unescape(hypothesis),
            })
        })
        .collect()
}

/// Aggregate counts over a parsed experiment log, for the status page's
/// ledger summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stats {
    /// Total parsed rows.
    pub total: usize,
    /// Rows with [`Verdict::Accepted`].
    pub accepted: usize,
    /// Rows with [`Verdict::Rejected`].
    pub rejected: usize,
    /// The chronologically last row, if any.
    pub latest: Option<Experiment>,
}

/// Summarizes `experiments` (in the file order [`parse_progress`]
/// returns) into [`Stats`].
#[must_use]
pub fn stats(experiments: &[Experiment]) -> Stats {
    let accepted = experiments
        .iter()
        .filter(|e| e.verdict == Verdict::Accepted)
        .count();
    Stats {
        total: experiments.len(),
        accepted,
        rejected: experiments.len() - accepted,
        latest: experiments.last().cloned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = concat!(
        r#"{"id": "it1", "date": "2026-08-20", "kind": "patch", "hypothesis": "first", "verdict": "accepted", "train_delta_bpb": null, "val_delta_bpb": null, "corpus": "N/A", "mechanism": "m", "commit": null}"#,
        "\n",
        r#"{"id": "it2", "date": "2026-08-21", "kind": "patch", "hypothesis": "a \"quoted\" idea", "verdict": "rejected", "train_delta_bpb": -0.1, "val_delta_bpb": 0.0, "corpus": "N/A", "mechanism": "m", "commit": null}"#,
    );

    #[test]
    fn parses_every_row() {
        let experiments = parse_progress(SAMPLE);
        assert_eq!(experiments.len(), 2);
        assert_eq!(experiments[0].id, "it1");
        assert_eq!(experiments[0].verdict, Verdict::Accepted);
        assert_eq!(experiments[1].id, "it2");
        assert_eq!(experiments[1].verdict, Verdict::Rejected);
    }

    #[test]
    fn unescapes_the_hypothesis() {
        let experiments = parse_progress(SAMPLE);
        assert_eq!(experiments[1].hypothesis, "a \"quoted\" idea");
    }

    #[test]
    fn skips_blank_lines() {
        let text = format!("\n{SAMPLE}\n\n");
        assert_eq!(parse_progress(&text).len(), 2);
    }

    #[test]
    fn drops_a_line_with_an_unknown_verdict() {
        let line = r#"{"id": "it3", "date": "2026-08-22", "verdict": "pending"}"#;
        assert_eq!(parse_progress(line).len(), 0);
    }

    #[test]
    fn stats_counts_accepted_and_rejected() {
        let experiments = parse_progress(SAMPLE);
        let s = stats(&experiments);
        assert_eq!(s.total, 2);
        assert_eq!(s.accepted, 1);
        assert_eq!(s.rejected, 1);
        assert_eq!(s.latest.as_ref().unwrap().id, "it2");
    }

    #[test]
    fn stats_on_empty_log() {
        let s = stats(&[]);
        assert_eq!(
            s,
            Stats {
                total: 0,
                accepted: 0,
                rejected: 0,
                latest: None
            }
        );
    }

    #[test]
    fn parses_the_real_progress_log_without_panicking() {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../research/progress.jsonl"),
        )
        .expect("research/progress.jsonl exists at the workspace root");
        let experiments = parse_progress(&text);
        assert!(!experiments.is_empty());
    }
}
