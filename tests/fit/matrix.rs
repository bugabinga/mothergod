//! What `tests/fit_cli.rs` and `tests/fit_lib.rs` share: the matrix file's
//! format and the two checks that keep a recorded state honest (ADR-0064).
//! Each includes this file with `#[path]`; a second parser would be a second
//! place for the format to drift.

use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Holds,
    Gap,
    Declined,
}

pub struct Row {
    pub id: &'static str,
    pub state: State,
    pub note: &'static str,
}

/// What a probe saw.
pub enum Seen {
    Holds,
    Fails,
    /// The lane cannot run the probe; the recorded state stands.
    Unmeasured,
}

pub fn seen(held: bool) -> Seen {
    if held { Seen::Holds } else { Seen::Fails }
}

pub type Probe = fn() -> Seen;

/// The rows of `matrix`, whose lines are `id | state | behavior | citation | note`.
pub fn rows(matrix: &'static str) -> Vec<Row> {
    matrix
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.splitn(5, " | ");
            let id = fields.next().expect("row has an id");
            let state = match fields.next().expect("row has a state") {
                "holds" => State::Holds,
                "gap" => State::Gap,
                "declined" => State::Declined,
                other => panic!("row {id}: state {other:?} is not holds, gap or declined"),
            };
            let _behavior = fields.next().expect("row has a behavior");
            let citation = fields.next().expect("row has a citation");
            assert!(!citation.trim().is_empty(), "row {id}: no citation");
            Row {
                id,
                state,
                note: fields.next().unwrap_or("").trim(),
            }
        })
        .collect()
}

/// Every row has one probe, a declined row has none, and a declined row says why.
pub fn check_probes_cover_rows(file: &str, matrix: &'static str, probes: &[(&str, Probe)]) {
    let rows = rows(matrix);
    let ids: BTreeSet<_> = rows.iter().map(|row| row.id).collect();
    assert_eq!(ids.len(), rows.len(), "duplicate row id in {file}");

    let probe_ids: BTreeSet<_> = probes.iter().map(|(id, _)| *id).collect();
    assert_eq!(probe_ids.len(), probes.len(), "a row is probed twice");

    let live: BTreeSet<_> = rows
        .iter()
        .filter(|row| row.state != State::Declined)
        .map(|row| row.id)
        .collect();
    assert_eq!(
        probe_ids, live,
        "probes and non-declined rows must be the same set"
    );

    for row in rows.iter().filter(|row| row.state == State::Declined) {
        assert!(
            !row.note.is_empty(),
            "declined row {} has no reason",
            row.id
        );
    }
}

/// Fails on every row whose recorded state disagrees with what its probe saw.
pub fn check_record(file: &str, matrix: &'static str, probes: &[(&str, Probe)]) {
    let recorded = rows(matrix);
    let mut stale = Vec::new();
    for (id, probe) in probes {
        let row = recorded
            .iter()
            .find(|row| row.id == *id)
            .unwrap_or_else(|| panic!("probe {id} has no row"));
        match (&row.state, probe()) {
            (State::Holds, Seen::Fails) => {
                stale.push(format!("{id}: recorded holds, it fails (a regression)"));
            }
            (State::Gap, Seen::Holds) => stale.push(format!(
                "{id}: recorded gap, it holds; flip the row to holds in {file}"
            )),
            _ => {}
        }
    }
    assert!(stale.is_empty(), "\n{}", stale.join("\n"));
}
