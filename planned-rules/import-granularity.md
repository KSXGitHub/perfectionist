# `import_granularity`

**Sources:** parallel-disk-usage *Code Style › Import Organization*; pacquet
*Import Organization*. Both source documents recommend the **merged** style;
the **module** and **separate** styles are supported here for projects that
prefer a different shape.

## Statement

A project picks one import-granularity style and enforces it consistently.
The three styles supported by this lint are:

- **`merged`** (default, matching both source documents): collapse every
  shared prefix into a single `use` statement. The end result is one `use`
  per top-level crate (per module body, per cfg gate). Example:
  `use std::{fs::{read as read_file, write as write_file}, io::{Error, ErrorKind}};`.
- **`module`**: keep one `use` per *leaf module*, but merge items inside
  that module into one braced list. Example:
  `use std::collections::{BTreeMap, BTreeSet};`.
- **`separate`**: every `use` statement imports exactly one leaf path.
  Example: `use std::collections::BTreeMap; use std::collections::BTreeSet;`.

Within each style the lint reports the same kind of mismatch: an import
block whose shape does not match the configured style.

## Configuration

```toml
# dylint.toml
[import_granularity]
style = "merged"   # or "module" or "separate"
```

Optional knobs (apply to all styles):

- `import_granularity.respect_cfg_blocks = true` — never merge across
  differing `#[cfg(...)]` attributes, and never merge across the
  trailing platform-gated block both source documents call out as
  separate.
- `import_granularity.respect_visibility = true` — never merge `pub use`
  with non-`pub` `use`.
- `import_granularity.respect_doc_comments = true` — never merge a
  `use` that carries its own `///` / `#[doc = "..."]` attribute.

## Style: `merged`

> Combine every shared prefix into a single `use` statement.

Two `use` statements are mergeable when they share at least one common
ancestor segment, sit in the same module body, and have matching
attributes/visibility (subject to the `respect_*` knobs above). The
result is one top-level `use` per crate root, with nested braces all the
way down.

```rust
// Bad (under style = "merged")
use std::path::Path;
use std::path::PathBuf;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};

// Good
use std::{
    collections::HashMap,
    io::{Error, ErrorKind},
    path::{Path, PathBuf},
};
```

## Style: `module`

Every `use` statement starts at a unique leaf module — i.e., the deepest
non-leaf segment of every import. Items pulled from the same leaf module
are merged into a single braced list; items from different modules
(even within the same crate) sit on separate `use` lines.

```rust
// Bad (under style = "module"): merged across modules
use std::{
    collections::HashMap,
    io::{Error, ErrorKind},
};

// Bad (under style = "module"): split within the same module
use std::collections::HashMap;
use std::collections::BTreeMap;

// Good
use std::collections::{BTreeMap, HashMap};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
```

A `use` statement counts as "module-style compliant" when its tree has
a single non-leaf path with one or more leaves directly under it. Nested
braces below the leaf module (`use foo::{bar::Baz, bar::Qux};`) are a
violation; the fix folds them into `use foo::bar::{Baz, Qux};`.

## Style: `separate`

Each `use` declaration imports exactly one leaf path. A project that picks
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

### Nested-list edge case (`separate` only)

A `use` like `use foo::{bar::Baz, bar::Qux};` has a *nested* brace list.
Under `separate` it must be split into two top-level `use` lines, not
flattened into `use foo::{bar::Baz}; use foo::{bar::Qux};`. The fix
removes the redundant braces.

## Implementation notes

- `EarlyLintPass::check_mod`. Walk the items, partition consecutive
  `ItemKind::Use` into compatible groups (matching attrs, cfg gates,
  visibility, surrounding doc comments), and apply the style-specific
  check to each group.
- The detection logic is shared across styles: build a tree keyed by
  path segments. The leaf count is the number of imported items, and
  the depth at which a `use` statement *starts* is the granularity
  cutoff.
  - Under `merged`, flag if the group of `use` statements maps to more
    than one top-level node when a single braced statement could
    represent the same set.
  - Under `module`, flag any `use` that either (a) crosses two distinct
    leaf modules at the top of its tree, or (b) splits items from the
    same leaf module across more than one `use` statement.
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
consistently:

| `import_granularity.style` | rustfmt `imports_granularity` |
|----------------------------|-------------------------------|
| `merged`                   | `Crate`                       |
| `module`                   | `Module`                      |
| `separate`                 | `Item`                        |

## Severity

Warn for all styles. None of the three styles is "wrong" in the
abstract; a mismatch with the project's configured style is the
violation.
