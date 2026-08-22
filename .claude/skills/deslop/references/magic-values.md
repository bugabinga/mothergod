# Magic values

Magic values are scalar literal values in code where it is not immediately apparent why they have their current value.

They are called magic because they often encode an assumption or a relationship to other values that happens to be correct at the time.

## What to look for

Look for scalar literal values whose meaning, origin, unit, or relationship to other values is unclear.

A common example is UI layout code:

```text
control.padding = 8
```

The value `8` may only look correct because another margin or spacing somewhere else happens to be `4`. If the relationship is “padding is twice the spacing”, the code should express that relationship instead of using a magic value.

## Why it matters

Magic values hide assumptions.

They can also be duplicated in multiple places and drift over time when one occurrence changes but another related occurrence does not.

## Preferred fix

When the value represents a relationship, encode the relationship directly.

When the relationship or context cannot be encoded, use a well-named constant.

Sometimes magic values are unavoidable because the context is not understood or the context exists outside the code. In those cases, a well-named constant is still better than an unexplained literal.

## When not to extract

Many literal values technically fit the definition of a magic value but are accepted in programmer culture.

For example, the zero index in a `for` loop is technically a scalar literal whose value matters, but there is no benefit in extracting it to a constant because every reader knows the pattern.

Extracting such values can make code worse. A shared constant for a loop start index could even introduce a bug if changing the constant changes all starting indices at once.

## Pseudo-code examples

### Bad: hidden relationship

```text
spacing = 4
control.padding = 8
```

The `8` only makes sense because it is related to `spacing`.

### Better: express the relationship

```text
spacing = 4
control.padding = 2 * spacing
```

This makes the assumption visible.

### Acceptable fallback: well-named constant

```text
minimum_visible_items = 7

if item_count < minimum_visible_items:
    show compact layout
```

If the context cannot be fully expressed in code, use a well-named constant.

### Bad extraction: culturally understood value

```text
first_index = 0

for index from first_index to item_count:
    process item[index]
```

This adds no benefit over the standard loop pattern.

### Worse: shared constant that can create drift or bugs

```text
start_index = 0

for index from start_index to item_count:
    process item[index]

for retry from start_index to retry_count:
    attempt retry[retry]
```

Changing `start_index` would affect unrelated loops at once.
