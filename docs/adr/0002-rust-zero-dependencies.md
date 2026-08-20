# ADR-0002: Rust, edition 2024, zero runtime dependencies in the core crate

Status: accepted · Date: 2026-08-20

## Context

The founding session ported the research prototype from Python to a
single-file Rust codec (v0.6) with no crates, which surfaced real port bugs
and retired the f64-determinism hazard via integer-only arithmetic. The
operator chose Rust for the project.

## Decision

- Implementation language: Rust, edition 2024, pinned stable toolchain.
- The core library crate has **zero runtime dependencies**. Dev-dependencies
  (test/bench/fuzz tooling) are allowed with reviewer scrutiny.
- Strict lints: `clippy::pedantic` + `missing_docs` warn, CI denies warnings.

## Rationale

A compressor is exactly the kind of code where supply-chain surface,
cross-platform determinism, and auditability matter more than convenience.
Zero deps keeps the trusted computing base equal to the compiler, keeps
compile times trivial for agent iteration loops, and matches the founding
implementation. It also forces agents to understand what they write.

## Consequences

We reimplement small utilities (bit I/O, hashing for match finders) ourselves.
Adding a runtime dependency later requires a superseding ADR.
