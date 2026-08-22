# Allocation discipline

CLAUDE.md hard rule 2: the decoder never overallocates unbounded, on ANY
input. This is the compression-bomb class, and it is the single easiest
way to ship a denial of service in a decompressor.

## The rule

**Never allocate a capacity derived from input without a bound.**

```text
let n = read_length(input)?;
let mut out = Vec::with_capacity(n);   // a 200-byte file can ask for 4 GiB
```

The length field is attacker-controlled. `with_capacity` believes it.

## Two bounds, and you need at least one

1. **Against remaining input.** For any structure whose size is bounded
   by the bytes that encode it, check the claim against what is actually
   left in the buffer before allocating. A header claiming ten thousand
   entries inside a two hundred byte file is a lie you can detect for
   free, before touching the allocator.

2. **Against a configured ceiling.** General-purpose compression has no
   input-derived bound on output size: that is the whole point of
   compression. So the output side needs an explicit maximum, and it is
   a format-level decision, which means it belongs in
   `docs/format/SPEC.md` and not invented per call site.

## Preferred shapes

- Grow incrementally. `Vec::new()` plus `extend_from_slice` as data
  actually arrives lets allocation fail progressively instead of on one
  enormous up-front reserve, and costs a few reallocations you will not
  measure.
- Reserve only what you have already read.
- Watch `.collect()`. Collecting an iterator driven by input length is
  an unbounded allocation wearing a functional-programming hat.
- Watch `vec![0u8; n]`, `String::with_capacity`, and `HashMap::with_capacity`
  for the same reason.

## Why not just catch the failure

You cannot. Allocation failure aborts by default in Rust; it is not a
catchable panic, and `try_reserve` only helps if you actually call it.
Bounding the request is the mechanism, not handling the failure.
