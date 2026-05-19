# `no_star_imports`

**Default state:** `active`  
**Source:** pacquet *No star imports*.

## Statement

Avoid star (glob) imports inside the bodies of regular modules. Two
exceptions are allowed by default, both individually configurable:

1. **External-crate preludes**, e.g. `use rayon::prelude::*;`,
   `use assert_cmd::prelude::*;`.
2. **Re-exports at the root of a module or crate**, e.g.
   `pub use submodule::*;` in `lib.rs`.

`use super::*;` inside a `#[cfg(test)] mod tests` block is the case the
guide is most concerned about; explicit imports must replace it.

A project that wants a stricter posture can disable either or both
exceptions through `dylint.toml`.

## What to lint

For every `use foo::bar::*;` (i.e., `UseTreeKind::Glob`), emit unless
one of the *enabled* exceptions applies:

- **`prelude`** (default enabled): the final non-glob segment of the
  path is `prelude`. Heuristic, but covers `rayon::prelude`,
  `assert_cmd::prelude`, `diesel::prelude`, etc. Configurable via
  `prelude_segment_names`.
- **`root_reexport`** (default enabled): the use is `pub` and sits at
  the *top level* of a module body.

When both exceptions are disabled the lint flags every glob `use`.

## Examples

```rust
// Bad
#[cfg(test)]
mod tests {
    use super::*;
}

// Good
#[cfg(test)]
mod tests {
    use super::{ParsedThing, parse_thing};
}
```

```rust
// Allowed by default (prelude exception)
use rayon::prelude::*;

// Allowed by default (root re-export)
pub use comver::*;
```

When `prelude` is disabled (`exceptions = ["root_reexport"]`):

```rust
// Bad (under that config)
use rayon::prelude::*;

// Good
use rayon::iter::{IntoParallelIterator, ParallelIterator};
```

When `root_reexport` is disabled (`exceptions = ["prelude"]`):

```rust
// Bad (under that config)
pub use comver::*;

// Good
pub use comver::{Version, VersionReq};
```

## Configuration

```toml
# dylint.toml
[no_star_imports]
# Which exception cases to allow. Both enabled by default.
exceptions = ["prelude", "root_reexport"]

# Names recognised as prelude segments.
prelude_segment_names = ["prelude"]

# Additional fully-qualified paths the lint should never flag, regardless
# of style. Useful for crate-specific glob conventions.
allowed_paths = []
```

A project that wants to ban *all* glob `use` statements outright can set:

```toml
[no_star_imports]
exceptions = []
```

## Implementation notes

- `EarlyLintPass::check_item` on `ItemKind::Use` with a tree containing
  a `Glob`.
- Determine the parent module via the item's `HirId` ancestors; the
  root-re-export exception requires the parent be the module root and
  the visibility be `pub`.
- For the prelude exception, inspect `UseTree::prefix` and check that
  the last segment ident is in `prelude_segment_names`.
- Read the `exceptions` config once per crate and store as a
  `bitflags`-style set; each `check_item` call consults it.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Interaction with [`named-prelude-import`](./named-prelude-import.md)

The two lints are duals. `no_star_imports` (with the `prelude`
exception enabled) lets `use foo::prelude::*;` through and forbids
`use super::*;`. `named_prelude_import` flags
`use foo::prelude::Item;` and lets `use foo::prelude::*;` through.
Together they say: "preludes must be glob-imported, and globs are only
allowed for preludes". Enabling both is the recommended posture for
projects that follow the prelude convention strictly.
