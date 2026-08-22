# Legacy and backwards-compatibility overengineering

Agents tend to read the current codebase into context. When they then receive instructions for new code, old code and new instructions are not always weighed correctly.

To weigh them correctly, the agent needs awareness about the state of the project and code.

## What to look for

Look for code or proposals that add or preserve complexity for reasons such as:

- legacy behavior
- backwards compatibility
- migrations
- versioning

These concerns may be important, or they may be useless noise, depending on the lifecycle stage of the project.

## Project and code state matters

In production, mission-critical, important, or released code, the old code may be very important. New code needs to take it into account.

In new features, prototypes, or unreleased projects, things like backwards compatibility, migrations, and versioning may be too early in the lifecycle and become useless noise.

## What the agent should verify

Before preserving or removing compatibility complexity, verify the state of the project and code:

- Is this production or mission-critical code?
- Is this released code?
- Is the behavior part of an external contract?
- Is this a new feature, prototype, or unreleased project?

## Pseudo-code examples

### Suspicious in an unreleased prototype

```text
if request version is 1:
    use old prototype behavior
else if request version is 2:
    use current prototype behavior
else:
    use fallback behavior
```

If the project is unreleased or still a prototype, versioning and backwards compatibility may be useless noise.

### Important in released production code

```text
if request version is 1:
    preserve released behavior
else if request version is 2:
    use current behavior
```

In released production code, old behavior may be important and new code must take it into account.

### Bad agent behavior: weighing old code blindly

```text
existing code has migration path
new feature copies migration path
```

This may be wrong if the new feature is unreleased and does not need migrations yet.

### Bad agent behavior: ignoring old code blindly

```text
new implementation replaces released behavior
old compatibility path removed without checking project state
```

This may be wrong if the code is production, mission-critical, important, or released.
