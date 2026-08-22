# Type precision

A codec is mostly `usize` values that mean incompatible things:

- byte offset into the input
- byte offset into the output, or into the window
- match length
- symbol id
- context or bucket index
- bit position within the coder

All six are the same type to the compiler. Swapping two of them
type-checks, compiles clean, passes clippy, and produces a bitstream
that decodes to the wrong bytes. The session-1 port bug was exactly this
shape: a rep-symbol and offset-bucket collision that existed because an
invariant lived only in one implementation's window size.

## The rule

**Newtype where two values of the same primitive can be confused and the
confusion would be silent.**

```text
struct WindowOffset(u32);
struct MatchLen(u16);
struct SymbolId(u8);
```

Free at runtime. `#[repr(transparent)]`, `Copy`, and the optimizer sees
straight through it. The only cost is conversion noise at the
boundaries, and that cost is diagnostic: if a function needs three
unwrapping conversions to do its arithmetic, the newtype boundary is in
the wrong place. Push it out to the API edge and let the interior work
in raw values.

Do not newtype everything. A loop counter is a loop counter.

## Related, from the Rust API Guidelines

- **C-NEWTYPE**: newtypes provide static distinctions.
- **C-CUSTOM-TYPE**: arguments convey meaning through types, not `bool`
  or `Option`. `encode(data, true, false)` is unreadable at the call
  site and unmisusable only by luck. An enum per axis says the same in
  less space.

These two are CLAUDE.md's precision value already codified by the Rust
library team. The rest of that checklist is written for public library
authors stabilising an API and mostly does not apply to us yet.

## Range as type

Where a value has a valid range, encode the range. A symbol id in
`0..256` is a `u8`, not a `usize`. An enum of five variants beats a
`&str` that secretly means one of five things: same meaning, less space,
every misuse closed at compile time.

Making illegal states unrepresentable is cheaper than testing that they
do not occur.
