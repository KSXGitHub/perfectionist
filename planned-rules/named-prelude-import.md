# `named_prelude_import`

**Source:** project convention. Dual of
[`no-star-imports`](./no-star-imports.md): that rule restricts globs in
general but lets preludes glob freely; this rule restricts named
imports *from* preludes and lets the glob form through.

## Statement

A `prelude` module is, by convention, a curated set of items that the
crate author has decided should always travel together as a glob. Cherry-
picking individual items from a prelude defeats that intent and usually
indicates that the importer should reach into the prelude's source
module instead.

This rule forbids any `use` statement whose path contains a `prelude`
segment *and* whose tree resolves to one or more named items rather
than a glob.

## What to lint

For every `use` statement, scan the use tree's path segments. If any
segment matches a prelude name (default: `prelude`), then:

- A `Glob` leaf below it is allowed.
- A `Simple` (named) leaf is flagged. Suggested fix: import the same
  item from its canonical source module.
- A nested brace list below the prelude (`use foo::prelude::{A, B};`)
  is flagged once per leaf.

The lint never fires on a `use` whose path contains no prelude
segment.

## Examples

```rust
// Bad
use serde::prelude::Serialize;
use serde::prelude::ser::Serialize;
use diesel::prelude::{table, AsChangeset};

// Good
use serde::Serialize;
use diesel::{table, AsChangeset};

// Allowed (the canonical prelude shape)
use serde::prelude::*;
```

## Why both lints together

`no-star-imports` (with the `prelude` exception) and
`named-prelude-import` together codify a single intent: "if a crate
ships a prelude, you import it as a glob; if you don't want the glob,
import the items from where they actually live." Enabling exactly one
of the two lints is also coherent:

- Only `no-star-imports`: globs are restricted, but you may still
  cherry-pick items from a prelude. (The default for projects that
  haven't opted into the convention.)
- Only `named-prelude-import`: glob `use` is unrestricted in general,
  but preludes must be glob-imported when used at all.

## Implementation notes

- `EarlyLintPass::check_item` on `ItemKind::Use`. Walk the
  `UseTree::prefix` segments and the leaves.
- The prelude detection is a simple ident match against
  `prelude_segment_names`, identical in shape to the corresponding
  knob in `no-star-imports`.
- For nested brace lists (`use serde::prelude::{A, B};`), expand each
  leaf into its own diagnostic span via the segment-walk machinery
  used by [`import-granularity`](./import-granularity.md). Reuse the
  same helper.
- Suggested fix: replace the `prelude::` segment with the canonical
  module of each item. Resolving the canonical module requires the
  item's `DefId` (`tcx.def_path` reports the *definition* path,
  bypassing re-exports), so this is a `LateLintPass` rather than an
  `EarlyLintPass`. Fix is `MachineApplicable` when the canonical path
  is unambiguous; `MaybeIncorrect` when the item is itself a re-export
  whose canonical path is in a private module.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Configuration

```toml
# dylint.toml
[named_prelude_import]
# Names recognised as prelude segments. Match `no-star-imports`'s knob
# of the same name so a project can flip both rules with one value.
prelude_segment_names = ["prelude"]

# Fully-qualified prelude paths the lint should never flag (e.g., a
# project's internal prelude that is intentionally cherry-picked).
allowed_paths = []
```

## Severity

Warn.
