# Security policy

## Reporting

Report vulnerabilities privately via
[GitHub Security Advisories](../../security/advisories/new).
Do **not** open a public issue. Reports are triaged by the human operator;
agents are not in the loop for undisclosed vulnerabilities.

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
