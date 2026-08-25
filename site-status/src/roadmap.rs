//! Parses ROADMAP.md's `## M<n> — <title>` milestone sections into
//! [`Milestone`]s for `site/status.html`'s milestone bar. Status is
//! derived mechanically from each section's own checkboxes (or the "✅"
//! ROADMAP.md already marks a finished milestone's heading with) rather
//! than asserted here, so this page can never claim a milestone finished
//! that ROADMAP.md itself still shows open — single source of truth.

/// One milestone section: its id (`"M0"`), title (the heading's text after
/// the em dash, the one-line scope `site/status.html` shows), and status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Milestone {
    /// Heading id, e.g. `"M0"`.
    pub id: String,
    /// Heading title after the em dash, `"✅"` stripped.
    pub title: String,
    /// Derived from the section's checkboxes; see [`parse_milestones`].
    pub status: Status,
}

/// A milestone's derived completion state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Heading carries "✅", or every checkbox in the section is checked.
    Done,
    /// At least one checkbox checked, but not all.
    Active,
    /// No checkbox checked (including a section with no checkboxes at
    /// all and no "✅").
    Pending,
}

impl Status {
    /// Lowercase name written into `site/status-data.json`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Active => "active",
            Self::Pending => "pending",
        }
    }
}

/// Parses every `## M<n> — <title>` section of `roadmap` (ROADMAP.md's
/// text) into a [`Milestone`], in document order. A section runs from its
/// heading to the next `## ` heading or end of file; only lines that
/// trim-start with `- [` count as checkboxes, so a soft-wrapped bullet's
/// continuation lines (ROADMAP.md wraps at ~80 columns) are never
/// double-counted.
#[must_use]
pub fn parse_milestones(roadmap: &str) -> Vec<Milestone> {
    let mut milestones = Vec::new();
    let mut lines = roadmap.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix("## M") else {
            continue;
        };
        let Some(digit_end) = rest.find(|c: char| !c.is_ascii_digit()) else {
            continue;
        };
        if digit_end == 0 {
            continue;
        }
        let id = format!("M{}", &rest[..digit_end]);
        let Some(title_part) = rest[digit_end..].strip_prefix(" — ") else {
            continue;
        };
        let done_heading = title_part.contains('\u{2705}');
        let title = title_part.trim_end_matches('\u{2705}').trim().to_string();

        let mut checked = 0usize;
        let mut total = 0usize;
        while let Some(&next) = lines.peek() {
            if next.starts_with("## ") {
                break;
            }
            if let Some(box_part) = next.trim_start().strip_prefix("- [") {
                total += 1;
                if box_part.starts_with("x]") {
                    checked += 1;
                }
            }
            lines.next();
        }

        let status = if done_heading || (total > 0 && checked == total) {
            Status::Done
        } else if checked > 0 {
            Status::Active
        } else {
            Status::Pending
        };

        milestones.push(Milestone { id, title, status });
    }
    milestones
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn done_heading_marks_done_with_no_checkboxes() {
        let roadmap = "## M0 — Scaffolding ✅\n\nProse, no checkboxes. Done 2026-08-20.\n";
        let milestones = parse_milestones(roadmap);
        assert_eq!(
            milestones,
            [Milestone {
                id: "M0".to_string(),
                title: "Scaffolding".to_string(),
                status: Status::Done,
            }]
        );
    }

    #[test]
    fn mixed_checkboxes_are_active() {
        let roadmap = "## M1 — Port the codec\n\n- [x] one\n- [ ] two\n";
        let milestones = parse_milestones(roadmap);
        assert_eq!(milestones[0].status, Status::Active);
    }

    #[test]
    fn no_checked_boxes_is_pending() {
        let roadmap = "## M6 — Release 0.1\n\n- [ ] one\n- [ ] two\n";
        let milestones = parse_milestones(roadmap);
        assert_eq!(milestones[0].status, Status::Pending);
    }

    #[test]
    fn all_checked_boxes_is_done() {
        let roadmap = "## M0 — Scaffolding\n\n- [x] one\n- [x] two\n";
        let milestones = parse_milestones(roadmap);
        assert_eq!(milestones[0].status, Status::Done);
    }

    #[test]
    fn section_with_no_checkboxes_and_no_mark_is_pending() {
        let roadmap = "## M3 — Close the gaps\n\nJust prose, no boxes.\n";
        let milestones = parse_milestones(roadmap);
        assert_eq!(milestones[0].status, Status::Pending);
    }

    #[test]
    fn soft_wrapped_bullet_continuation_is_not_a_second_checkbox() {
        let roadmap = "## M1 — Port the codec\n\n- [x] Founding artifacts imported to the\n      archive, verified lossless.\n";
        let milestones = parse_milestones(roadmap);
        assert_eq!(milestones[0].status, Status::Done);
    }

    #[test]
    fn stops_at_the_next_heading() {
        let roadmap = "## M1 — First\n\n- [ ] a\n\n## M2 — Second\n\n- [x] b\n";
        let milestones = parse_milestones(roadmap);
        assert_eq!(milestones.len(), 2);
        assert_eq!(milestones[0].status, Status::Pending);
        assert_eq!(milestones[1].status, Status::Done);
    }

    #[test]
    fn non_milestone_headings_are_skipped() {
        let roadmap = "## Mission\n\nNot a milestone.\n\n## M0 — Real one ✅\n";
        let milestones = parse_milestones(roadmap);
        assert_eq!(milestones.len(), 1);
        assert_eq!(milestones[0].id, "M0");
    }

    #[test]
    fn parses_the_real_roadmap_without_panicking() {
        let roadmap = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ROADMAP.md"),
        )
        .expect("ROADMAP.md exists at the workspace root");
        let milestones = parse_milestones(&roadmap);
        assert!(
            milestones.len() >= 6,
            "expected M0..M6 at least, got {milestones:?}"
        );
        assert_eq!(milestones[0].id, "M0");
        assert_eq!(milestones[0].status, Status::Done);
    }
}
