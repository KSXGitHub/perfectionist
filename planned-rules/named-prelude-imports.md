# `named_prelude_imports`

**Source:** project convention. Dual of
`perfectionist::wildcard_imports` (`src/rules/wildcard_imports.rs`): that
rule restricts globs in general but lets preludes glob freely; this rule
restricts named imports *from* preludes and lets the glob form through.

## Status

Implemented in `src/rules/named_prelude_imports.rs`, active by default.
What is done:

- Detection of every cherry-picked named import from a prelude segment,
  including each leaf of a brace list (`use foo::prelude::{A, B};`),
  flagged once per leaf.
- Both configuration knobs (`prelude_segment_names`, `allowed_paths`).
- The canonical-module autofix for **standalone** imports
  (`use foo::prelude::Item;` → `use <canonical>::Item;`), resolved from
  the item's `DefId`, preserving any `as` rename and gauging
  `MachineApplicable` vs. `MaybeIncorrect` by whether the canonical path
  is publicly nameable.

What is **not** done (this file stays until it is):

- The mechanical fix for a **brace-list leaf**
  (`use foo::prelude::{A, B};`). Each leaf is still flagged, but only
  carries a `help`, not a `span_suggestion`. A correct fix can't rewrite
  a single leaf's sub-span in place — the leaves may resolve to
  different canonical modules, so the statement has to split into one
  `use` per leaf, which is `import_granularity`-shaped work left for a
  follow-up.

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

**Avoid:**

```rust
use serde::prelude::Serialize;
use serde::prelude::ser::Serialize;
use diesel::prelude::{table, AsChangeset};
```

**Prefer:**

```rust
use serde::Serialize;
use diesel::{table, AsChangeset};
```

**Not flagged:** the canonical prelude shape.

```rust
use serde::prelude::*;
```

## Why both lints together

`perfectionist::wildcard_imports` (with the `prelude` exception) and
`named_prelude_imports` together codify a single intent: "if a crate
ships a prelude, you import it as a glob; if you don't want the glob,
import the items from where they actually live." Enabling exactly one
of the two lints is also coherent:

- Only `wildcard_imports`: globs are restricted, but you may still
  cherry-pick items from a prelude. (The default for projects that
  haven't opted into the convention.)
- Only `named_prelude_imports`: glob `use` is unrestricted in general,
  but preludes must be glob-imported when used at all.

## Implementation notes

As built (see `src/rules/named_prelude_imports.rs`):

- A plain HIR `LateLintPass::check_item` on
  `ItemKind::Use(path, UseKind::Single(_))`. The fix needs the item's
  `DefId`, which only exists post-resolution, and a HIR walk already
  reaches every compiled module (including separate-file submodules), so
  the re-parse machinery the source-layout import rules use is not
  needed. HIR lowers `use serde::prelude::{A, B};` into one
  `UseKind::Single` item per leaf, so each brace-list leaf is visited —
  and flagged — individually with no flattening of our own (the original
  plan of reusing `import_granularity`'s `model.rs` leaf flattening was
  unnecessary).
- The prelude detection is a simple ident match against
  `prelude_segment_names`, identical in shape to the corresponding
  knob in `perfectionist::wildcard_imports`.
- Fix: replace the written path span with the item's canonical module
  path, built from `tcx.def_path` (the *definition* path, bypassing
  re-exports) prefixed with `crate` for the local crate or the crate
  name otherwise. Applicability is `MachineApplicable` when every
  component up to the crate root is `pub` (so the path is nameable from
  any importer) and `MaybeIncorrect` otherwise. **Only standalone
  imports are auto-fixed** — see the Status section for why brace-list
  leaves carry a `help` instead.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Configuration

```toml
# dylint.toml
[named_prelude_imports]
# Names recognised as prelude segments. Match `wildcard_imports`'s knob
# of the same name so a project can flip both rules with one value.
prelude_segment_names = ["prelude"]

# Fully-qualified prelude paths the lint should never flag (e.g., a
# project's internal prelude that is intentionally cherry-picked).
allowed_paths = []
```

## Default state

Active by default.
