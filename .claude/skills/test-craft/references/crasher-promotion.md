# Crasher promotion

A fuzz artifact or torture failure is a bug report from a machine.
This procedure turns it into a fix plus a permanent regression seed.

1. **Reproduce on current main**: `cargo fuzz run <target>
   <artifact>`. A crasher that no longer reproduces still gets its
   seed committed; the input reached a state nothing else reaches.
2. **Minimize**: `cargo fuzz tmin <target> <artifact>`. The committed
   seed should document a boundary, not a haystack.
3. **Fix the bug.** The fix and the seed land in one PR. The Rust
   mechanics of the fix (allocation bounds, panic discipline) are
   rust-craft's territory.
4. **Promote the seed** into `tests/adversarial/`: kebab-case, no
   extension, named for the failure mode, never the finder
   (`bad-magic-bitflip`, `lz-declared-size-bomb`; never
   `crash-<hash>`). Prefix with the area (`lz-`) when the case is
   frame-specific. The harness `tests/adversarial.rs` sweeps the
   whole directory; there is no per-seed test to write.
5. **Name the bug where the filename cannot**: the seed's name
   carries the failure mode, the PR body carries the mechanism. If
   the name cannot carry it alone, add one line to
   `tests/adversarial.rs`'s module doc.

Every promoted seed is held to layer 2's contract: graceful `Err`,
bounded allocation, never a panic. If the crasher demonstrates a
class the harness does not yet assert (an allocation bound, say),
extend the harness, not a one-off test.
