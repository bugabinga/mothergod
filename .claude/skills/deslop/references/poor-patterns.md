# Poor patterns

Poor patterns include blindly applying SOLID, Uncle Bob-style “Clean Code”, or
old OOP patterns when they add ceremony, indirection, or hidden costs instead of
making the system simpler.

## What to look for

Look for code shaped by patterns such as:

- SOLID applied mechanically
- “Clean Code” rules applied mechanically
- deep type hierarchies
- reference types for everything
- ignoring memory and performance implications in the name of “maintainability”

## Why it matters

Patterns can make code worse when they add indirection, allocation, dispatch,
hierarchy, or conceptual weight without a real need.

Maintainability does not mean ignoring memory and performance implications.

Prefer composition over inheritance when behavior can be assembled directly instead of encoded in a type hierarchy.

## SOLID anti-patterns

SOLID concepts can be useful, but applied mechanically they often create slop.

### Single Responsibility Principle

Anti-pattern: splitting code until every tiny operation becomes a separate class, service, or function, even when the pieces do not make sense independently.

```text
class UserNameTrimmer
class UserEmailLowercaser
class UserAgeValidator
class UserNormalizerCoordinator
```

This may satisfy a narrow idea of “one responsibility” while making behavior harder to read as a whole.

### Open/Closed Principle

Anti-pattern: adding extension points, inheritance, plugins, or registries before there is a real need for variation.

```text
interface PriceRule
class DefaultPriceRule implements PriceRule
class PriceRuleRegistry
class PriceRuleFactory
```

This can make simple code harder to change because future flexibility was guessed too early.

### Liskov Substitution Principle

Anti-pattern: deep inheritance hierarchies where subclasses technically share a parent type but violate expectations through special cases, unsupported operations, or surprising overrides.

```text
class Storage
class ReadOnlyStorage extends Storage:
    function write(data):
        throw unsupported_operation
```

The type relationship claims substitutability, but the behavior does not support it.

### Interface Segregation Principle

Anti-pattern: creating many tiny interfaces that add names, files, dispatch, and navigation without reducing real coupling.

```text
interface CanGetName
interface CanSetName
interface CanValidateName
interface CanNormalizeName
```

Small interfaces are not automatically simple if they fragment one concept across many places.

### Dependency Inversion Principle

Anti-pattern: wrapping every direct dependency in an interface, adapter, provider, or factory even when there is only one implementation and no useful boundary.

```text
interface ClockProvider
class SystemClockProvider implements ClockProvider
class ClockProviderFactory
class ClockProviderFactoryProvider
```

This can replace a clear dependency with ceremony and indirection.

## Pseudo-code examples

### Bad: deep type hierarchy

```text
interface Thing
class AbstractThing implements Thing
class AbstractNamedThing extends AbstractThing
class ConfigurableNamedThing extends AbstractNamedThing
class RuntimeConfigurableNamedThing extends ConfigurableNamedThing
class UserRuntimeConfigurableNamedThing extends RuntimeConfigurableNamedThing
```

The hierarchy becomes the thing the reader must understand before understanding
the behavior.

### Better: flatter data and behavior

```text
type Thing:
    name
    config

function run_thing(thing, runtime):
    use thing.name
    use thing.config
    use runtime
```

Prefer a simpler shape when the hierarchy does not carry real value.

### Bad: inheritance for assembled behavior

```text
class Exporter
class CsvExporter extends Exporter
class CompressedCsvExporter extends CsvExporter
class EncryptedCompressedCsvExporter extends CompressedCsvExporter
```

The type hierarchy encodes combinations of behavior.

### Better: composition over inheritance

```text
exporter = compose(
    csv_format,
    compression,
    encryption,
)

exporter.export(data)
```

Assemble behavior directly when that is simpler than encoding combinations in subclasses.

### Bad: reference types for everything

```text
class UserName:
    value

class UserAge:
    value

class UserEmail:
    value

class User:
    name: UserName reference
    age: UserAge reference
    email: UserEmail reference
```

Reference types for everything can add allocation, navigation, and runtime cost
without improving the system.

### Better: use direct values when appropriate

```text
type User:
    name
    age
    email
```

Do not introduce reference-heavy structure unless it has a real purpose.

### Bad: ceremony in the name of maintainability

```text
interface UserFactory
class DefaultUserFactory implements UserFactory
class UserFactoryProvider
class UserFactoryProviderFactory

user = UserFactoryProviderFactory.create().provider().factory().create_user(data)
```

This may satisfy pattern rules while making the actual behavior harder to see.

### Better: direct construction when enough

```text
user = create_user(data)
```

Avoid pattern ceremony when direct code is simpler and sufficient.

### Bad: ignoring performance implications

```text
function draw_frame(items):
    objects = items.map(item -> new DrawableItemWrapper(item))
    objects.each(object -> object.render())
```

A maintainability argument is weak if the code creates avoidable memory or
performance costs in a hot path.

### Better: consider the runtime context

```text
function draw_frame(items):
    for each item in items:
        render item
```

When memory or performance matters, do not hide costs behind abstraction.
