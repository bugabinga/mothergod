# Security policy

## Reporting

Report vulnerabilities privately via
[GitHub Security Advisories](../../security/advisories/new).
Do **not** open a public issue. The human operator alone confirms a
candidate is real, publishes an advisory, requests a CVE, and triggers
the release. Under
[ADR-0032](docs/adr/0032-private-defensive-security-lane.md), a
BDFL-directed agent may privately draft a candidate advisory, and, once
the operator confirms it, prepare a fix in the advisory's private fork.
None of this is ever public: no candidate detail reaches an issue, PR,
branch, commit, workflow log, or chat message before a release exists.

## Threat model (what counts as a vulnerability here)

mothergod decodes attacker-controlled input by design. In scope:

- Decoder panic, crash, or undefined behavior on any input (malformed,
  truncated, bit-flipped, adversarially constructed).
- Decompression bombs: output or memory not bounded by declared limits.
- Round-trip violations: any input where `decompress(compress(x)) != x`.
- Non-deterministic output across platforms for the same input and version
  (a format-integrity hazard).

## Supported versions

Pre-1.0: only the latest release is supported. The bitstream format is
unstable until `FORMAT_VERSION` 1 is frozen (see `docs/format/SPEC.md`).
