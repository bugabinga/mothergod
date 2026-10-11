# ADR-0066: The CLI speaks the genre's grammar

Status: accepted · Date: 2026-10-11 · Applies ADR-0064 · Serves issue #983

## Context

The CLI was `mothergod <compress|decompress> [file]`.
The six tools of the genre (gzip, bzip2, xz, zstd, brotli, lz4) compress by default, take `-d` to decompress and `-c` to write to stdout, and process any number of file operands.
No genre tool has a subcommand word, so `mothergod -dc x.mgdc | tar x` failed and a user's reflexes transferred nothing (#983).
ADR-0064 expected the grammar to replace the subcommand shape and promised a record when it shipped.

## Decision

The command line is `mothergod [-d] [-c] [--] [file ...]`, with `-h` and `-V` answering at once.
Short options cluster (`-dc`), `--decompress` and `--stdout` are the long forms, `--` ends options, and the operand `-` names stdin.
With no operand the run is stdin to stdout, compressing unless `-d` is given.
With a file operand the output is the file with the suffix added (compress) or stripped (`-d`), unless `-c` sends it to stdout.
Operands are processed in order; a failure on one is reported and the run continues, then exits 1.
A decode failure names its input, because several operands make the message the only clue.
A reader that goes away ends the run at once with status 141, as before.

The `compress` and `decompress` words are retired with no alias: the genre is unanimous, and a file named `compress` would otherwise change meaning by position.
They now read as file operands and fail to read a file that does not exist.
Unknown options, including the genre's flags this slice does not implement yet (`-k`, `-f`, `-t`, `-l`, `-q`, `-v`, levels), are usage errors, not ignored.

This slice flips the matrix rows `mode-d`, `stdout-c`, `end-of-options`, `stdin-dash`, `bare-compresses` and `multiple-operands` in `tests/fit/cli.txt`.
The remaining #983 clusters (`-k` and `-f`, `-t` and `-l`, `-q`, `-v` and exit codes) add flags to this parser, each flipping its own rows.

## Consequences

An interface break before any release (M6 is unreleased), so no user holds the old shape.
README and the site's recipe change to `mothergod < FILE > FILE.mgdc` and `mothergod -d < FILE.mgdc > FILE.out`; `tests/claims.rs` checks each published option against `--help`.
The compression refusal of a terminal now covers `-c` and the `-` operand as well as no operand, wherever stdout is the destination.

## Rejected alternatives

**Keep the subcommands as aliases beside the flags.**
Two grammars for one behavior are two surfaces to test and document, and the aliases serve only the users of a shape that never shipped.

**A parsing dependency (clap, lexopt).**
The core crate and its binary take no runtime dependencies (ADR-0002), and the grammar is a dozen lines.
