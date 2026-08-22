# Poor error handling

Poor error handling can mean both handling too broadly and not handling broadly enough.

## What to look for

Poor error handling includes:

- handling too broad a set of errors and reacting the same way to all of them
- handling too narrow a set of errors and ignoring relevant error cases

## Local or deferred handling

Error handling may be local or deferred, depending on project rules.

The important point is not where the error is handled, but that wherever it is handled, the handling itself is useful, actionable, and explains context to humans.

## Who the error is for

Some errors concern developers.

Some errors concern end users.

Depending on the case, different handling may be needed:

- language
- wording style
- logging
- quality of explanation
- technical detail level

## Error messages

When crafting error messages, pay attention to:

- context
- technical details
- localization

## Pseudo-code examples

### Bad: too broad, same reaction for everything

```text
try:
    load configuration
    connect to database
    read user input
catch error:
    show "Something went wrong"
```

Different failures are collapsed into the same reaction, losing useful context.

### Better: distinguish relevant cases

```text
try:
    load configuration
    connect to database
    read user input
catch configuration_error:
    report developer-facing configuration context
catch database_error:
    report connection context and useful technical details
catch input_error:
    report user-facing input guidance
```

The handling gives humans useful and actionable context.

### Bad: too narrow, error case ignored

```text
result = parse input
use result
```

If parsing can fail, the error case is ignored.

### Better: handle or defer according to project rules

```text
result = parse input
if result is parse_error:
    handle here with useful context
    or return error with context for caller to handle

use result
```

Handling can be local or deferred, but the eventual handling must be useful.

### Developer-facing vs user-facing

```text
if migration file is invalid:
    log technical details for developers
    fail startup with configuration context

if user entered invalid date:
    show localized user-facing guidance
```

Different audiences require different language, detail, logging, and wording style.
