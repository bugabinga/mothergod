# Overengineered error handling

Overengineered error handling relates to deep nesting, but it deserves its own reference because many languages have diverse and different error handling strategies.

## What to look for

Overengineered error handling includes error handling constructs that are:

- misused for control flow
- deeply nested
- reliant on side effects

## Why it matters

Error handling can make code harder to understand when it obscures the normal behavior of the code or turns exceptional paths into the main control structure.

## Pseudo-code examples

### Bad: error handling used as control flow

```text
try:
    value = get cached value
catch missing_value_error:
    value = calculate value
```

The missing cache value is part of normal control flow, but it is expressed as an error path.

### Better: explicit control flow

```text
if cache has value:
    value = get cached value
else:
    value = calculate value
```

Normal behavior is visible as normal control flow.

### Bad: deeply nested error handling

```text
try:
    open resource
    try:
        read resource
        try:
            parse resource
        catch parse_error:
            handle parse error
    catch read_error:
        handle read error
catch open_error:
    handle open error
```

The nesting makes the error behavior hard to read.

### Better: flatter structure

```text
resource = open resource
if open failed:
    handle open error
    return

content = read resource
if read failed:
    handle read error
    return

parsed = parse content
if parse failed:
    handle parse error
    return
```

Flattening can make the normal sequence and error cases easier to follow.

### Bad: error handling reliant on side effects

```text
try:
    update account
catch validation_error:
    validation_error handler also updates account state
    continue
```

The handler changes state in a way the reader may not expect.

### Better: make side effects explicit

```text
validation = validate account update
if validation failed:
    update account state for failed validation
    report validation error
    return

update account
```

The side effect is visible in normal code instead of hidden inside error handling.
