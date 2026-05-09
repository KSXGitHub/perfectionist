# `no_star_imports`

**Source:** pacquet *No star imports*.

## Statement

Avoid star (glob) imports inside the bodies of regular modules. Two
exceptions:

1. **External-crate preludes**, e.g. `use rayon::prelude::*;`,
   `use assert_cmd::prelude::*;`.
2. **Re-exports at the root of a module or crate**, e.g.
   `pub use submodule::*;` in `lib.rs`.

`use super::*;` inside a `#[cfg(test)] mod tests` block is the case the
guide is most concerned about; explicit imports must replace it.

## What to lint

For every `use foo::bar::*;` (i.e., `UseTreeKind::Glob`), emit unless one
of the following holds:

- The use is `pub` and sits at the *top level* of a module body
  (root-of-module re-export).
- The final non-glob segment of the path is `prelude` (prelude exception).
  This is a heuristic but covers `rayon::prelude`, `assert_cmd::prelude`,
  `diesel::prelude`, etc.

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
// Allowed (prelude exception)
use rayon::prelude::*;

// Allowed (root re-export)
pub use comver::*;
```

## Implementation notes

- `EarlyLintPass::check_item` on `ItemKind::Use` with a tree containing a
  `Glob`.
- Determine the parent module via the item's `HirId` ancestors; the
  root-re-export exception requires the parent be the module root and the
  visibility be `pub`.
- For the prelude exception, inspect `UseTree::prefix` and check that the
  last segment ident is `prelude`.

## Configuration

Provide a `dylint.toml` knob `no_star_imports.allowed_paths` so projects
can extend the prelude allowlist (e.g., `tracing::prelude`) without
patching the lint.

## Severity

Warn.
