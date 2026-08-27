# x

`cargo x` is mothergod's agent-facing quality interface.
It embeds repository formatters and linters behind one discoverable command.

```text
cargo x help
cargo x help fmt
cargo x help lint
```

## Commands

```text
cargo x check
cargo x fmt [--check] [PATH...]
cargo x lint [--fix] [PATH...]
cargo x test
cargo x doc
```

`check` is the whole gate: `fmt --check`, `lint`, `test`, then `doc`,
unscoped, in that order. It stops at the first failing stage and names the
command to re-run just that one. It is the one pre-push command CLAUDE.md
names.

`test` runs the fixed plan CLAUDE.md's Commands block names as the test
suites: `cargo test --all-targets`, `cargo test --manifest-path
x/Cargo.toml`, then `cargo test --doc`, in that order. It is not
file-scoped; it stops at the first failing suite and names the command to
re-run just that one.

`doc` runs `RUSTDOCFLAGS="--deny warnings" cargo doc --no-deps`, the
CLAUDE.md doc gate. It is not file-scoped either; on failure it names the
command to re-run.

No paths selects every supported tracked file.
A file selects itself.
A directory recursively selects tracked and non-ignored untracked files.
Explicit files may be new and untracked.
Missing, excluded, unsupported, and symbolic-link paths are errors, never silent skips.
Archived files under `research/imports/` remain untouched.

Rust formatting is file-scoped.
Rust linting widens a selected file to its containing Cargo package because Clippy's semantic unit is a package.
The separate nightly `fuzz/` workspace remains outside x lint.
Every other operation remains file-scoped.

| Files | `fmt` | `lint` |
|---|---|---|
| Rust | rustfmt | Clippy |
| Markdown | | rumdl with the repository profile below |
| JSON | serde_json canonical formatting | |
| JSONL | serde_json syntax plus final-newline normalization | |
| TOML | Taplo | |
| YAML | pretty_yaml | |
| JavaScript and TypeScript | dprint TypeScript | |
| HTML and SVG | markup_fmt | |

Python, shell, and GitHub Actions semantics have no reliable embedded Rust linter in this tool.
Use explicit paths to receive a concrete unsupported-file error rather than assuming coverage.

## Agent contract

The API is conventional and stable:

- `cargo x help` lists tasks;
- every task supports `--help` with scope, defaults, and examples;
- `--` separates options from paths;
- success output stays concise;
- diagnostics name the path, location when available, problem, and next command;
- invocation and configuration errors fail immediately;
- file findings are aggregated so one run exposes the whole repair set.

Exit codes are stable:

- `0`: success;
- `1`: formatting or lint findings;
- `2`: invalid invocation, configuration, or tool failure.

`cargo x lint --fix` applies only fixes rumdl marks safe.
Clippy remains check-only.
The task checks the fixed Markdown again and reports anything still unresolved.

### Markdown profile

Rumdl's default rules apply except opt-in rules and these repository conflicts:

| Rules | Repository convention |
|---|---|
| MD004, MD018, MD032 | continuation lines and issue references resemble list or heading syntax |
| MD013 | semantic line breaks replace a fixed line length |
| MD024, MD076 | changelog sections repeat and group list entries |
| MD033, MD041 | README and templates intentionally start with HTML or comments |
| MD034 | `agents/SOURCES.md` is a bare-URL inventory |
| MD040 | text and terminal fences do not claim a programming language |
| MD057 | GitHub-root-relative links cannot resolve on the local filesystem |

The exclusions live in `x/src/lint.rs` beside the executable rule selection.

## Narrow checks and hooks

Path arguments are the integration boundary for agents and hooks.
During iteration, pass only the files or directories being changed.
Before a push, run `cargo x check`.

Git integrations must produce NUL-delimited paths with `git diff --name-only -z --diff-filter=ACMR`.
`git diff --stat` is display output, not a safe filename protocol.
A hook can pass those paths after `--`; it must handle an empty path set without accidentally invoking an unscoped check.

## Performance

`x` is a standalone workspace with its own lockfile and target directory.
The compressor workspace never compiles x's formatter dependency graph.
Local Cargo builds x once and reuses it.
CI caches the exact x binary by operating system, architecture, lockfile, manifest, and source hash; a cache hit runs tasks without rebuilding x.
Only tooling selected by the resolved file set is initialized.
