# Deep nesting

Deep nesting is a good place to start analyzing code slop.

## What to look for

Look for code that is deeply indented to the right, especially:

- deeply nested `if` / `else` chains, for example more than 3 levels deep
- nested `switch` statements
- nested `for` loops
- combinations of nested conditionals, switches, and loops

The “more than 3 levels” rule is a simple starting point, not a universal rule. It depends on context.

## Why it matters

A simple but useful metric is how far the code is indented to the right.

The more code is pushed to the right, the harder it usually is to read and understand.

## Preferred fixes

Guard clauses early in functions and early returns are often good techniques for reducing nesting.

Extracting functions may be good, but be careful: extracting a function that is used only once is only worth it if the extracted code is inherently complex and can be given a good name.

Splitting phases, lookup tables, and state machines are fine techniques, but the user must be made aware of the tradeoffs in memory and space-time differences compared to the old code.

## When it can be acceptable

Deep nesting can be appropriate and correct in some contexts, for example:

- text parsers
- binary parsers
- algorithms

Even in these cases, deep nesting is still a good starting point for analysis.

## Pseudo-code examples

### Bad: nested condition chain

```text
function process(input):
    if input exists:
        if input is valid:
            if user has permission:
                if mode is supported:
                    do work
                else:
                    return unsupported mode
            else:
                return no permission
        else:
            return invalid input
    else:
        return missing input
```

### Better: guard clauses and early returns

```text
function process(input):
    if input does not exist:
        return missing input

    if input is not valid:
        return invalid input

    if user does not have permission:
        return no permission

    if mode is not supported:
        return unsupported mode

    do work
```

### Possible but needs care: extracted single-use function

```text
function process(input):
    if input cannot be processed:
        return reason

    do work

function input cannot be processed:
    complex validation with a good name
```

Only extract a single-use function when the extracted code is inherently complex and the function can be given a good name.

### Possible but tradeoffs must be explained: lookup table or state machine

```text
function process(event):
    next_step = transition_table[current_state][event]
    run next_step
```

This may reduce visible nesting, but the user must be told about memory and space-time tradeoffs compared to the old code.
