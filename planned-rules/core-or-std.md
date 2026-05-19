# `core_or_std`

**Default state:** `active`  
**Source:** project convention.

## Statement

A project picks one style for naming items that exist in both `core` (or
`alloc`) and `std`, and enforces it consistently. The three styles
supported by this lint are:

- **`prefer_core`** — flag any `std::` path that names an item canonically
  defined in `core` or `alloc`. Suggest the narrower path. Mirrors
  `clippy::std_instead_of_core` and `clippy::std_instead_of_alloc`,
  both of which live in clippy's `restriction` group (off by default).
- **`prefer_std`** — flag any `core::` or `alloc::` path that names an
  item also reachable through `std::`. Suggest the `std::` path.
- **`preserve`** (default) — no-op. Useful as a project-wide
  acknowledgement that the rule exists but neither extreme is enforced.

The choice is project-driven:

- A library that wants to keep the door open to `no_std` (or that
  already conditions on `#![no_std]`) wants `prefer_core` so the path
  text is portable as-is.
- An application or library that targets `std` exclusively often wants
  `prefer_std` for stylistic consistency: every `use` line begins with
  the same crate.

## Configuration

```toml
# dylint.toml
[core_or_std]
style = "preserve"   # or "prefer_core" or "prefer_std"
```

Optional knobs:

- `core_or_std.also_alloc = true` — defaults to `true`. When `false`,
  the lint ignores `alloc`-vs-`std` differences and only governs
  `core`-vs-`std`. Useful for projects that depend on `std`
  permanently but want to track `core` cleanliness for future
  portability.
- `core_or_std.skip_paths = []` — fully-qualified path strings that the
  lint should never flag, regardless of style. Useful for items that
  re-export inconsistently across rustc versions.

## Style: `prefer_core`

Flag any path whose written first segment is `std` but whose resolved
`DefId` belongs to the `core` or `alloc` crate.

```rust
// Bad (under style = "prefer_core")
use std::fmt::Display;
use std::collections::HashMap;   // re-exported from `alloc::collections`

// Good
use core::fmt::Display;
use alloc::collections::BTreeMap;
```

`std`-only items (`std::fs`, `std::io`, `std::net`, `std::process`,
`std::sync::Mutex`, `std::thread`, `std::env`, …) are never flagged
under any style — there is no narrower path to suggest.

## Style: `prefer_std`

Flag any path whose written first segment is `core` or `alloc` and
suggest the matching `std::` path.

```rust
// Bad (under style = "prefer_std")
use core::fmt::Display;
use alloc::sync::Arc;

// Good
use std::fmt::Display;
use std::sync::Arc;
```

The lint should *not* fire inside a module gated by `#![no_std]` or
`#[cfg(not(feature = "std"))]`, because `std::` is unavailable there.
Detect via `tcx.sess.contains_name(.., sym::no_std)` on the crate's
attributes for the global case, and by walking enclosing items for the
`cfg`-gated case.

## Style: `preserve`

The lint emits nothing. Default.

## Examples across both styles

| Path                          | `prefer_core` | `prefer_std` |
|-------------------------------|---------------|--------------|
| `std::option::Option`         | flag → `core::option::Option` | ok |
| `std::vec::Vec`               | flag → `alloc::vec::Vec` (with `also_alloc = true`) | ok |
| `std::fs::read`               | ok (std-only) | ok |
| `core::fmt::Display`          | ok            | flag → `std::fmt::Display` |
| `alloc::sync::Arc`            | ok            | flag → `std::sync::Arc` |

## What to lint

- `LateLintPass::check_path` (and `check_use_tree` via `check_item` on
  `ItemKind::Use`).
- For each path:
  1. Take the first written segment. If it is not `std`, `core`, or
     `alloc`, ignore.
  2. Resolve the path's `DefId`.
  3. Look up `tcx.crate_name(def_id.krate)`. This is the canonical
     crate of the item, regardless of the path the user wrote.
  4. Apply the configured style:
     - `prefer_core`: written `std` and canonical `core`/`alloc` → flag.
     - `prefer_std`: written `core`/`alloc` and item reachable through
       `std::` → flag. Reachability is "true for every public item in
       `core` and `alloc`" — both crates are wholly re-exported through
       `std` since Rust 1.36 (and earlier for `core`).

## Implementation notes

- Use `clippy_utils::ty::match_def_path` to confirm the canonical path
  for any specific item, but for the general case `def_id.krate` plus
  the written first segment is enough.
- The autofix substitutes only the first path segment. Render via
  `clippy_utils::source::snippet_with_applicability`.
  `Applicability::MachineApplicable` for the substitution; downgraded
  to `MaybeIncorrect` when the path appears inside a macro expansion
  whose tokens cannot be cleanly rewritten.
- For `prefer_std` mode, the rewrite assumes `extern crate alloc;` is
  no longer needed once the path is `std`-prefixed. The lint does
  *not* offer to remove `extern crate alloc;` declarations
  automatically — that requires whole-crate analysis the lint pass
  does not perform.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Interaction with clippy

- `prefer_core` is functionally equivalent to enabling
  `clippy::std_instead_of_core` and `clippy::std_instead_of_alloc`.
  If a project already has both clippy lints set to `warn` or `deny`,
  this lint should detect that and downgrade itself to allow (or be
  disabled via `dylint.toml`).
- `prefer_std` has no clippy equivalent; the perfectionist
  implementation is the canonical one.

## Default state

The default `style = "preserve"` keeps the
pass a no-op until the project opts into `prefer_core` or
`prefer_std`.
