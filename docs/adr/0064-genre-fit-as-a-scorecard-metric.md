# ADR-0064: Genre fit as a scorecard metric

Status: accepted · Date: 2026-10-09 · Prompted by the operator's request for a best-in-class CLI and library (Telegram, 2026-10-09)

## Context

`ROADMAP.md`'s product shape puts mothergod in the zstd/xz/gzip genre: one CLI that compresses and decompresses, and a library crate, both table stakes because that shape is what a human choosing a compressor compares against.
The scorecard judges the codec (RATIO, SPEED), its safety (TRUST), its adoption (USERS) and its size (SIMPLICITY).
Nothing judges whether the CLI or the library fit the hand that reaches for them, though MISSION.md's third non-negotiable makes ease of integrating a first-class outcome.

Measured against the genre on 2026-10-09 (dev build, main at 68b2aad): `mothergod --version` is an unknown command (#977); `mothergod decompress | head -c 1` exits 1 with a broken-pipe error where gzip exits in silence (#976); concatenated frames decode to the first payload alone with exit 0 (#978); the six genre tools share one flag grammar and the CLI shares none of it (#983).
The library (`docs/api-surface.txt`, 22 items) has no `std::io::Read` or `Write` adapter, the front-page surface of every compression crate a Rust user has used (#979).

A metric that cannot be measured is itself a top gap (MISSION.md).
"Best-in-class" judged by taste is a claim without a run, which this project rejects everywhere else.

## Decision

The scorecard gains an outcome metric, **FIT**: the share of the genre's behaviors the CLI and the library hold, measured by a conformance suite and published as a count.

The genre is the set a user compares against: gzip, bzip2, xz, zstd, brotli and lz4 for the CLI; flate2, zstd, xz2, brotli and lz4_flex for the library.
A behavior enters the matrix on the genre's evidence alone, with its citation on the row (a man page section, the POSIX utility syntax guidelines, a crate's documented surface, the Rust API Guidelines), never because mothergod happens to do it.
Where the genre splits, the row records the split and the project's choice with its reason.

Each row is one test with a recorded state, `holds` or `gap`.
A `holds` row that fails is a regression and fails the gate.
A `gap` row that passes fails the gate until the record flips.
A row the project declines keeps its place in the denominator with the reason on the row.
FIT is holds over total, so the number cannot drift silently in either direction and cannot be raised by adding rows.

A row holds only where it holds on every runtime lane that can run the suite; a platform with no lane is unmeasured, and the gap is the lane, not the row.
The count is published the way SIMPLICITY's API surface is: a committed file the gate keeps honest, read by `status-data.py`, shown on /status with trend.

Milestone M8, Genre fit, holds the matrix and the gaps it names.
Its first slice is the measurement; no gap closes before the row that measures it exists.

## Consequences

The CLI's grammar becomes a decision the matrix forces rather than a taste: the genre's `-d`, `-c`, `-k`, `-f`, `-t` grammar is expected to replace the subcommand shape (#983).
That is an interface change and gets its own record when it ships (ADR-0030).
Some rows need format work, a frame-stream rule (#978) or an integrity check (#982), and ride hard rule 5 when built.

The suite spawns the binary, so it runs on no Miri or Android lane, and BSD has no lane at all (#980).
The cost is a test layer (`docs/TESTING.md` layer 12), a committed count, a status field and a milestone of items, bounded by the genre's own size: the matrix is finite, so M8 has a finish line where M3 and M5 do not.

## Rejected alternatives

Judge fit in review.
Unmeasurable, and a reviewer's "feels right" is the unearned authority the scorecard exists to replace.

A user study.
Unavailable to a project with no release and costly forever; the genre is the accumulated user study, thirty years of people reaching for the same flags.
