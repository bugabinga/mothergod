# Complexity

Complexity is not about lines of code. It is a property of the system.

## Simple vs. complex

Simple means one role, one task, one objective, or one concept. It is not about
being easy or familiar.

Complex means intertwined, braided together, or complected.

Easy means near, familiar, or already in the developer’s skillset. Easy is not
the same as simple.

## What to look for

Look for things that are intertwined and cannot be reasoned about independently.

At code level, pay attention to dependencies created by:

- function parameters
- side effects
- class parameters
- module parameters

At project level, pay attention to dependencies created by:

- libraries
- vendored code
- assumptions about the operating system
- assumptions about the runtime

## Why it matters

Complexity makes systems harder to understand, change, debug, and rely on.

A system can be short, familiar, or easy to write and still be complex if
concerns are braided together.

## Pseudo-code examples

### Bad: function entangled with too many dependencies

```text
function calculate_invoice(order, user, database, logger, clock, config, runtime_environment):
    read user discount from database
    check current time from clock
    branch on runtime_environment
    write audit log
    calculate total
    update order state
    return total
```

This function ties calculation to storage, time, logging, runtime assumptions,
and mutation.

### Better: keep calculation separate from effects

```text
function calculate_invoice(order, discount, current_time, rules):
    calculate total from values
    return total

function save_invoice(order, total, database, logger):
    update order state
    write audit log
```

The calculation can be reasoned about separately from side effects.

### Bad: project-level runtime assumption hidden in code

```text
function find_tool():
    return "/usr/bin/tool"
```

The code silently assumes a specific operating system and runtime environment.

### Better: make the dependency visible

```text
function find_tool(runtime_config):
    return runtime_config.tool_path
```

The dependency on the runtime environment is explicit.

### Bad: easy but complex

```text
function process(data):
    import convenient_big_library
    convenient_big_library.do_everything(data)
```

This may be easy to write, but it can add project-level complexity through a
library dependency and its assumptions.

### Better: inspect dependency tradeoffs

```text
function process(data, required_operation):
    required_operation(data)
```

Before adding or keeping a dependency, analyze what it intertwines with the
project: libraries, vendored code, operating system assumptions, and runtime
assumptions.
