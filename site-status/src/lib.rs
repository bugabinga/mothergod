//! Builds `site/status-data.json`: the honest-vitals snapshot
//! `site/status.html` renders (issue #95). Prose fields (`phase`,
//! `works_today`, the benchmarks note) are hand-kept strings, same as
//! `site/index.html`'s status box; everything else is parsed straight out
//! of the files it must never contradict — the milestone bar reads
//! ROADMAP.md's own checkboxes ([`roadmap`]) rather than a hand-kept
//! duplicate of them, and the experiment ledger reads
//! `research/progress.jsonl` ([`experiments`]) — single source of truth.
//!
//! Remaining scope (`research/JOURNAL.md` S2-D1's "progress-graph
//! rendering" line): the scheduled `.github/workflows/site-status-data.yml`
//! that reruns `site-status/src/bin/generate.rs` and commits the result
//! needs `GH_ADMIN_TOKEN` to push a workflow file
//! (`agents/GOVERNANCE.md`, "Push identity"), not available to the session
//! that wrote this crate; until it lands, `site/status-data.json` is a
//! manually regenerated snapshot, not an automatically fresh one, and its
//! `generated_at` stamp is how a reader tells. The HEALTH scorecard metric
//! (agent session success rate from GitHub Actions) is also deferred: it
//! needs the Actions API, unlike everything else here, which reads only
//! this checkout.

pub mod experiments;
pub mod json;
pub mod roadmap;

use std::fmt::Write as _;

/// Everything [`render`] needs to produce `site/status-data.json`.
#[derive(Debug, Clone)]
pub struct StatusData {
    /// UTC timestamp this snapshot was generated, `%Y-%m-%dT%H:%M:%SZ`.
    pub generated_at: String,
    /// Project lifecycle phase, e.g. `"pre-alpha"`.
    pub phase: String,
    /// [`mothergod::FORMAT_VERSION`] at generation time.
    pub format_version: u8,
    /// One-line description of which [`mothergod::Method`] variants exist
    /// and round-trip today.
    pub works_today: String,
    /// Milestone bar, in ROADMAP.md order.
    pub milestones: Vec<roadmap::Milestone>,
    /// Honest state of the benchmarks section (ROADMAP M2: no
    /// Silesia/Canterbury bits/byte vs gzip/zstd/xz exists yet).
    pub benchmarks_note: String,
    /// `research/progress.jsonl` summary.
    pub experiments: experiments::Stats,
    /// Commits in the last 7 days whose subject matches this repo's
    /// squash-merge convention (`... (#<number>)`), a merged-PR proxy.
    pub merged_prs_7d: usize,
    /// `research/progress.jsonl` rows dated in the last 7 days.
    pub recorded_experiments_7d: usize,
    /// Lines across `src/**/*.rs`, the SIMPLICITY scorecard proxy.
    pub sloc_src: usize,
    /// `#[test]`-attributed functions across `src/**/*.rs`.
    pub test_functions_src: usize,
}

/// Renders `data` as the pretty JSON object `site/status-data.json`
/// commits, 2-space indented with a trailing newline (`bench::baseline`'s
/// `format_baseline` style).
#[must_use]
#[allow(clippy::too_many_lines)] // one flat object, no natural place to split
pub fn render(data: &StatusData) -> String {
    let mut out = String::from("{\n");
    writeln!(
        out,
        "  \"generated_at\": \"{}\",",
        json::escape(&data.generated_at)
    )
    .expect("writing to a String never fails");
    writeln!(out, "  \"phase\": \"{}\",", json::escape(&data.phase))
        .expect("writing to a String never fails");
    writeln!(out, "  \"format_version\": {},", data.format_version)
        .expect("writing to a String never fails");
    writeln!(
        out,
        "  \"works_today\": \"{}\",",
        json::escape(&data.works_today)
    )
    .expect("writing to a String never fails");

    out.push_str("  \"milestones\": [\n");
    for (idx, m) in data.milestones.iter().enumerate() {
        let comma = if idx + 1 == data.milestones.len() {
            ""
        } else {
            ","
        };
        writeln!(
            out,
            "    {{ \"id\": \"{}\", \"title\": \"{}\", \"status\": \"{}\" }}{comma}",
            json::escape(&m.id),
            json::escape(&m.title),
            m.status.as_str()
        )
        .expect("writing to a String never fails");
    }
    out.push_str("  ],\n");

    writeln!(
        out,
        "  \"benchmarks_note\": \"{}\",",
        json::escape(&data.benchmarks_note)
    )
    .expect("writing to a String never fails");

    out.push_str("  \"experiments\": {\n");
    writeln!(out, "    \"total\": {},", data.experiments.total)
        .expect("writing to a String never fails");
    writeln!(out, "    \"accepted\": {},", data.experiments.accepted)
        .expect("writing to a String never fails");
    writeln!(out, "    \"rejected\": {},", data.experiments.rejected)
        .expect("writing to a String never fails");
    if let Some(latest) = &data.experiments.latest {
        writeln!(out, "    \"latest_id\": \"{}\",", json::escape(&latest.id))
            .expect("writing to a String never fails");
        writeln!(
            out,
            "    \"latest_hypothesis\": \"{}\"",
            json::escape(&latest.hypothesis)
        )
        .expect("writing to a String never fails");
    } else {
        out.push_str("    \"latest_id\": null,\n");
        out.push_str("    \"latest_hypothesis\": null\n");
    }
    out.push_str("  },\n");

    writeln!(out, "  \"merged_prs_7d\": {},", data.merged_prs_7d)
        .expect("writing to a String never fails");
    writeln!(
        out,
        "  \"recorded_experiments_7d\": {},",
        data.recorded_experiments_7d
    )
    .expect("writing to a String never fails");
    writeln!(out, "  \"sloc_src\": {},", data.sloc_src).expect("writing to a String never fails");
    writeln!(out, "  \"test_functions_src\": {}", data.test_functions_src)
        .expect("writing to a String never fails");

    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use experiments::{Stats, Verdict};

    fn sample() -> StatusData {
        StatusData {
            generated_at: "2026-08-25T00:00:00Z".to_string(),
            phase: "pre-alpha".to_string(),
            format_version: 2,
            works_today: "Stored and Lz".to_string(),
            milestones: vec![roadmap::Milestone {
                id: "M0".to_string(),
                title: "Scaffolding".to_string(),
                status: roadmap::Status::Done,
            }],
            benchmarks_note: "not yet measurable".to_string(),
            experiments: Stats {
                total: 1,
                accepted: 1,
                rejected: 0,
                latest: Some(experiments::Experiment {
                    id: "it1".to_string(),
                    date: "2026-08-20".to_string(),
                    verdict: Verdict::Accepted,
                    hypothesis: "a \"quoted\" idea".to_string(),
                }),
            },
            merged_prs_7d: 3,
            recorded_experiments_7d: 5,
            sloc_src: 5420,
            test_functions_src: 200,
        }
    }

    #[test]
    fn renders_and_escapes_the_latest_hypothesis() {
        let text = render(&sample());
        assert!(text.contains("\"latest_hypothesis\": \"a \\\"quoted\\\" idea\""));
        assert!(text.contains("\"id\": \"M0\""));
        assert!(text.contains("\"status\": \"done\""));
    }

    #[test]
    fn renders_null_latest_on_an_empty_ledger() {
        let mut data = sample();
        data.experiments = Stats {
            total: 0,
            accepted: 0,
            rejected: 0,
            latest: None,
        };
        let text = render(&data);
        assert!(text.contains("\"latest_id\": null"));
        assert!(text.contains("\"latest_hypothesis\": null"));
    }

    #[test]
    fn every_line_of_a_two_milestone_render_is_valid_json_ish_shape() {
        let mut data = sample();
        data.milestones.push(roadmap::Milestone {
            id: "M1".to_string(),
            title: "Port the codec".to_string(),
            status: roadmap::Status::Active,
        });
        let text = render(&data);
        // No trailing comma before a closing bracket/brace.
        assert!(!text.contains(",\n  ]"));
        assert!(!text.contains(",\n}"));
    }
}
