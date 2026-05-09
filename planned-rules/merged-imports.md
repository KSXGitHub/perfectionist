# `merged_imports`

**Sources:** parallel-disk-usage *Code Style › Import Organization*; pacquet
*Import Organization*.

## Statement

> Combine multiple items from the same crate or module into a single `use`
> statement with braces rather than separate `use` lines.

## What to lint

Within one contiguous import block (same module body, same `cfg` gate, same
visibility), flag any pair of `use` declarations that share the longest-prefix
ancestor and could be merged into one braced `use`.

Two `use` statements are mergeable when:

- They have the same `Visibility` (both `use` or both `pub use`).
- They carry the same set of attributes (in particular, the same `cfg`).
- Their paths share at least one common ancestor segment.

The lint should *not* merge across `cfg` boundaries or across the platform-
gated trailing block, because both source documents explicitly carve those
out as separate blocks.

## Examples

```rust
// Bad
use std::path::Path;
use std::path::PathBuf;
use std::collections::HashMap;

// Good
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
```

## Implementation notes

- `EarlyLintPass::check_mod`. Iterate items, group consecutive
  `ItemKind::Use` with matching attrs/visibility, build a tree keyed by
  path segments, and emit when any internal node has more than one child
  spread across multiple top-level `use` items.
- `clippy_utils::source::snippet_with_applicability` for rendering the
  combined replacement.
- The autofix is mechanical for the common case (no renaming, no glob
  inside the merged set). Suggest with `Applicability::MachineApplicable`
  when paths are simple, otherwise `MaybeIncorrect`.

## Interaction with `cargo fmt`

`rustfmt`'s `imports_granularity` and `group_imports` options can do this
automatically, but they are unstable. The lint exists to enforce the rule
even on stable toolchains.

## Severity

Warn.
