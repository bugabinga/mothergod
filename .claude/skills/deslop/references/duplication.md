# Duplication

DRY is generally a good guideline.

Duplicated code risks drift and makes fixing bugs harder.

## What to look for

Look for repeated code or repeated logic that may need to change together over time.

Duplication is more suspicious when a bug fix or behavior change would likely
need to be applied to every copy.

## Key question

Before removing duplication, ask whether the repeated code should actually
change together over time.

If yes, deduplication may reduce drift and make fixes safer.

If no, the code may only be coincidentally the same right now. In that case,
extracting a shared abstraction can be wrong.

## When duplication may be acceptable

Duplication may be acceptable when the repeated code is independent and only
coincidentally the same right now.

Do not force unrelated code to share an abstraction just because it currently
looks similar.

## Pseudo-code examples

### Bad: duplicated logic that should change together

```text
function price_for_web(order):
    subtotal = sum order items
    tax = subtotal * tax_rate
    total = subtotal + tax
    return total

function price_for_invoice(order):
    subtotal = sum order items
    tax = subtotal * tax_rate
    total = subtotal + tax
    return total
```

If pricing rules must change together, the duplication risks drift.

### Better: shared behavior when changes should stay together

```text
function calculate_price(order):
    subtotal = sum order items
    tax = subtotal * tax_rate
    total = subtotal + tax
    return total

function price_for_web(order):
    return calculate_price(order)

function price_for_invoice(order):
    return calculate_price(order)
```

This is appropriate when both callers should share the same pricing behavior
over time.

### Acceptable: coincidentally similar code

```text
function retry_network_request():
    wait 100 milliseconds
    try again

function delay_tooltip_open():
    wait 100 milliseconds
    show tooltip
```

The same literal delay does not prove the behavior should change together.

### Bad abstraction: forcing independent behavior together

```text
function wait_standard_delay():
    wait 100 milliseconds

function retry_network_request():
    wait_standard_delay()
    try again

function delay_tooltip_open():
    wait_standard_delay()
    show tooltip
```

If network retry timing and tooltip timing are independent, this shared
abstraction can create future bugs.
