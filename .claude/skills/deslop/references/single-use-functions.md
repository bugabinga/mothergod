# Single-use functions

Single-use functions are often bad because they force names onto blocks of code.

That can sometimes be a benefit, but only when the name is very good: short, descriptive, and unambiguous.

## Why it matters

Single-use functions force readers to jump and navigate around the codebase to understand behavior.

In some languages, they can also incur performance or runtime costs.

## When a single-use function can be good

For complex or surprising code, giving the block a good name can be the right move.

This only applies when the function name is very good:

- short
- descriptive
- unambiguous

More often than not, comments can serve the same purpose better.

## Preferred fix

Inline single-use functions when they do not justify the navigation cost.

If the old function name was useful, translate it into a human-readable comment above the inlined code.

## Tricky case: public functions

Single-use public functions require careful analysis.

They may be part of an API and only appear to be single-use from the current codebase.

Analyze those cases case by case before proposing an inline.

## Pseudo-code examples

### Bad: single-use function with weak name

```text
function handle_request(request):
    prepare(request)
    send response

function prepare(request):
    trim request fields
    default missing values
    normalize date format
```

The reader must jump to `prepare` to understand what actually happens.

### Better: inline with a useful comment

```text
function handle_request(request):
    # Normalize request input before responding.
    trim request fields
    default missing values
    normalize date format

    send response
```

If the function name carried useful intent, convert that intent into a human-readable comment.

### Acceptable: complex or surprising block with a very good name

```text
function handle_request(request):
    reject_if_signature_was_replayed(request)
    send response

function reject_if_signature_was_replayed(request):
    complex or surprising replay-detection logic
```

A single-use function may be worth keeping when the code is complex or surprising and the name is short, descriptive, and unambiguous.

### Risky: public single-use function

```text
public function normalize_request(request):
    trim request fields
    default missing values
    normalize date format
```

This may be an API even if it appears single-use inside the current codebase. Analyze case by case before proposing to inline it.
