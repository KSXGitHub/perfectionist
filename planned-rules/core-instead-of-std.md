# `core_instead_of_std`

**Source:** project convention.

## Statement

A project that targets `std` exclusively can enforce that every item
reachable through `std::` is named through `std::`, never through its
narrower `core::` / `alloc::` origin, so every `use` line begins with the
same crate.

The lint flags any path whose written first segment is `core` or `alloc`
when the item it names is also reachable through `std::`, and suggests
the `std::` path.

The *opposite* direction — preferring the narrower `core::` / `alloc::`
path — is intentionally out of scope here: it is already covered exactly
by `clippy::std_instead_of_core` and `clippy::std_instead_of_alloc`, both
in clippy's `restriction` group (off by default). A library that wants to
keep the door open to `no_std` enables those two clippy lints; this rule
is the `std`-preferring counterpart they have no equivalent for.

## Configuration

```toml
# dylint.toml
#
# Inactive by default. Enable in `[perfectionist].enable`. The rule has
# a single direction (prefer `std::`), so there is no `style` knob.
[core_instead_of_std]
# Defaults to `true`. When `false`, the lint ignores `alloc`-vs-`std`
# differences and only governs `core`-vs-`std`. Useful for projects that
# depend on `std` permanently but want to track `core` cleanliness for
# future portability.
also_alloc = true

# Fully-qualified path strings the lint should never flag. Useful for
# items that re-export inconsistently across rustc versions.
skip_paths = []
```

## What it flags

Flag any path whose written first segment is `core` or `alloc` and
suggest the matching `std::` path.

**Avoid:**

```rust
use core::fmt::Display;
use alloc::sync::Arc;
```

**Prefer:**

```rust
use std::fmt::Display;
use std::sync::Arc;
```

| Path                  | flagged?                          |
|-----------------------|-----------------------------------|
| `core::fmt::Display`  | flag → `std::fmt::Display`        |
| `alloc::sync::Arc`    | flag → `std::sync::Arc` (with `also_alloc = true`) |
| `std::fs::read`       | ok (already `std`)                |

The lint should *not* fire inside a module gated by `#![no_std]` or
`#[cfg(not(feature = "std"))]`, because `std::` is unavailable there.
Detect via `tcx.sess.contains_name(.., sym::no_std)` on the crate's
attributes for the global case, and by walking enclosing items for the
`cfg`-gated case.

## What to lint

- `LateLintPass::check_path` (and `check_use_tree` via `check_item` on
  `ItemKind::Use`).
- For each path:
  1. Take the first written segment. If it is not `core` or `alloc`,
     ignore.
  2. Resolve the path's `DefId`.
  3. The item is reachable through `std::` for every public item in
     `core` and `alloc` — both crates are wholly re-exported through
     `std` since Rust 1.36 (and earlier for `core`) — so a written
     `core` / `alloc` first segment is enough to flag (subject to
     `also_alloc` for the `alloc` case).

## Implementation notes

- The autofix substitutes only the first path segment. Render via
  `clippy_utils::source::snippet_with_applicability`.
  `Applicability::MachineApplicable` for the substitution; downgraded
  to `MaybeIncorrect` when the path appears inside a macro expansion
  whose tokens cannot be cleanly rewritten.
- The rewrite assumes `extern crate alloc;` is no longer needed once the
  path is `std`-prefixed. The lint does *not* offer to remove
  `extern crate alloc;` declarations automatically — that requires
  whole-crate analysis the lint pass does not perform.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Default state

Inactive by default. Targeting `std` exclusively is a project decision —
a `no_std` library wants the opposite — so the rule ships no baseline;
enable it in `[perfectionist].enable`.
