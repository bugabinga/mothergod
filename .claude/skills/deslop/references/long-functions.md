# Long functions

Long functions are not inherently a problem.

They can offer the benefit that complex logic can be read top to bottom and understood completely.

## What to look for

A long function, for example more than about 50 lines, can be a good starting point for analysis.

Do not treat length alone as proof of slop.

## Questions to ask

When analyzing a long function, ask:

- Can duplicated parts be extracted and reused?
- Does the function partly reimplement existing functions or behavior?
- Can the function be simplified or optimized without sacrificing behavior?
- Should the function be split for better readability?

## When splitting may help

Splitting a long function for readability is not often the right answer, but
sometimes it helps.

Examples where splitting may be more understandable:

- separating the happy path from error handling
- separating layout / UI code from behavior

General rule: separating by clear boundaries in narrow domains of behavior

This is rare and should be proposed carefully.

## Pseudo-code examples

### Not automatically bad: long top-to-bottom logic

```text
function calculate_result(input):
    prepare data
    validate data
    apply rule A
    apply rule B
    apply rule C
    combine result
    return result
```

This may be acceptable if the logic is easier to understand when read top to bottom.

### Candidate for extraction: duplicated parts

```text
function render_page_a(data):
    normalize title
    normalize date
    format author
    render page A

function render_page_b(data):
    normalize title
    normalize date
    format author
    render page B
```

The duplicated normalization / formatting may be a candidate for extraction and
reuse.

### Candidate for simplification: reimplemented behavior

```text
function process(items):
    result = []
    for each item in items:
        if item matches condition:
            result add transformed item
    return result
```

If the project already has an existing function or behavior for this, the long
function may be partly reimplementing it.

### Rare but possible: split happy path from error handling

```text
function save(input):
    if input invalid:
        handle validation error
        return

    if storage unavailable:
        handle storage error
        return

    save input
    return success
```

If the error handling makes the main behavior hard to see, separating the happy
path from error handling may improve readability.

### Rare but possible: split UI layout from behavior

```text
function screen():
    create layout
    attach button behavior
    create more layout
    attach form behavior
    create final layout
```

If layout and behavior obscure each other, splitting them may be more understandable.
