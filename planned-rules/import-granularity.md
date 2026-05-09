# `import_granularity`

**Sources:** parallel-disk-usage *Code Style › Import Organization*; pacquet
*Import Organization*. Both source documents recommend the **merged** style;
the **separate** style is supported here as the inverse for projects that
prefer one `use` statement per imported item.

## Statement

A project picks one import-granularity style and enforces it consistently.
The two styles supported by this lint are:

- **`merged`** (default, matching both source documents): combine multiple
  items from the same crate or module into a single `use` statement with
  braces. Flag separate `use` lines that share a common prefix.
- **`separate`**: every `use` statement imports exactly one path. Flag any
  `use` statement whose top-level brace list contains more than one item.

Within each style the lint reports the same kind of mismatch: an import
block that does not match the configured shape.

## Configuration

```toml
# dylint.toml
[import_granularity]
style = "merged"   # or "separate"
```

Optional knobs (apply to both styles):

- `import_granularity.respect_cfg_blocks = true` — never merge across
  differing `#[cfg(...)]` attributes, and never merge across the
  trailing platform-gated block both source documents call out as
  separate.
- `import_granularity.respect_visibility = true` — never merge `pub use`
  with non-`pub` `use`.
- `import_granularity.respect_doc_comments = true` — never merge a
  `use` that carries its own `///` / `#[doc = "..."]` attribute.

## Style: `merged`

> Combine multiple items from the same crate or module into a single
> `use` statement with braces rather than separate `use` lines.

Two `use` statements are mergeable when they share at least one common
ancestor segment, sit in the same module body, and have matching
attributes/visibility (subject to the `respect_*` knobs above).

```rust
// Bad (under style = "merged")
use std::path::Path;
use std::path::PathBuf;
use std::collections::HashMap;

// Good
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
```

## Style: `separate`

Each `use` declaration imports exactly one path. A project that picks
this style usually does so to make `git blame` and code review easier
(one-line diffs per import change) and to side-step rustfmt's quirks
around brace-list reflow.

A `use` statement counts as "separate-style compliant" when its tree is a
single non-glob path leaf, optionally renamed with `as`. A glob is *not*
a violation under `separate`; globs are governed by the
[`no_star_imports`](./no-star-imports.md) lint.

```rust
// Bad (under style = "separate")
use std::path::{Path, PathBuf};
use std::collections::{HashMap, BTreeMap};

// Good
use std::path::Path;
use std::path::PathBuf;
use std::collections::HashMap;
use std::collections::BTreeMap;
```

The autofix for `separate` mode synthesises one `use` per leaf, copying
the original visibility and attributes onto each new line.

### Nested-list edge case

A `use` like `use foo::{bar::Baz, bar::Qux};` has a *nested* brace list.
Under `separate` it must be split into two top-level `use` lines, not
flattened into `use foo::{bar::Baz}; use foo::{bar::Qux};`. The fix
removes the redundant braces.

## Implementation notes

- `EarlyLintPass::check_mod`. Walk the items, partition consecutive
  `ItemKind::Use` into compatible groups (matching attrs, cfg gates,
  visibility, surrounding doc comments), and apply the style-specific
  check to each group.
- The detection logic is shared between styles: build a tree keyed by
  path segments. The leaf count is the number of imported items.
  - Under `merged`, flag if the group of `use` statements maps to more
    than one top-level node when a single braced statement could
    represent the same set.
  - Under `separate`, flag any `use` whose tree has more than one leaf.
- Use `clippy_utils::source::snippet_with_applicability` to render the
  replacement.
- Suggest with `Applicability::MachineApplicable` when no `as` renames
  collide and no macro expansions overlap the spans; otherwise
  `MaybeIncorrect`.

## Interaction with `cargo fmt`

`rustfmt`'s `imports_granularity` and `group_imports` options can
enforce the same shape, but they are unstable. This lint exists to give
stable-toolchain projects an alternative, and to fire as a hard CI check
rather than as a silent reformat.

If a project enables both rustfmt's option and this lint, configure them
to the same value (`Crate` ⇔ `merged`, `Item` ⇔ `separate`).

## Severity

Warn for both styles. Neither style is "wrong" in the abstract; a
mismatch with the project's configured style is the violation.
