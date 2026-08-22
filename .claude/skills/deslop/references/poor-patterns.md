# Poor patterns

Patterns applied by reflex, not by need: SOLID, "Clean Code", and
classic OOP patterns deployed where they add ceremony, indirection, or
hidden cost instead of removing any.

A pattern is a tool with a price: indirection, allocation, dispatch,
hierarchy, conceptual weight. Reflex application pays the price without
collecting the benefit. "Maintainability" is not an argument when
nothing gets easier to maintain, and it never excuses ignoring memory
and performance costs.

## The tells

Every SOLID and Clean-Code anti-pattern is the same defect wearing a
different principle: **structure added for variation, substitution, or
extension that does not exist yet and may never come.** The surface
signals:

- names that describe structure, not behavior: Factory, Provider,
  Registry, Coordinator, Manager, Abstract-anything
- an interface or trait with exactly one implementation and nothing
  else, not even a test double, using the seam
- a type hierarchy deeper than two, or one that encodes combinations
  of behavior as subtypes
- extension points, plugins, or registries with a single registrant
- one concept fragmented across many tiny interfaces or classes, so
  that no single piece makes sense alone
- a subtype that technically satisfies its parent but throws, no-ops,
  or surprises on part of the contract
- reference or heap types for everything, costs hidden behind
  abstraction in paths where they matter

## Two examples stand for the catalog

Ceremony instead of construction:

```text
interface UserFactory
class DefaultUserFactory implements UserFactory
class UserFactoryProvider
class UserFactoryProviderFactory

user = UserFactoryProviderFactory.create().provider().factory().create_user(data)
```

versus `user = create_user(data)`.

Hierarchy instead of composition:

```text
class Exporter
class CsvExporter extends Exporter
class CompressedCsvExporter extends CsvExporter
class EncryptedCompressedCsvExporter extends CompressedCsvExporter
```

versus assembling the three behaviors at the call site:

```text
exporter = compose(csv_format, compression, encryption)
```

The hierarchy makes the reader understand the tree before the behavior,
and every new combination costs a new subtype.

## Preferred fix

Direct construction, direct calls, flat data. Compose behavior at the
call site instead of encoding combinations in a hierarchy. Delete the
layer whose only job is delegating to the next layer, and keep deleting
until every remaining name denotes behavior a reader can observe. Add
the seam back on the day a second implementation actually exists.
