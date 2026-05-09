# `derive_ordering`

**Source:** parallel-disk-usage *Derive Macro Ordering* (pacquet does not
restate the rule, but does not contradict it). The exact intra-category
order parallel-disk-usage prefers is, objectively, somewhat arbitrary —
no semantic difference comes from putting `Debug` before `Default`
versus the reverse. This lint is therefore configurable; the project
picks a style, and the parallel-disk-usage shape is recoverable as one
configuration.

## Statement

The lint has two independent sub-lints, each with its own `style`:

1. **`derive_ordering::within_attribute`** — order of trait names
   *inside* a single `#[derive(...)]` list.
2. **`derive_ordering::cross_attribute`** — how derives are
   *partitioned* across multiple `#[derive(...)]` lines (or
   `#[cfg_attr(.., derive(..))]` lines).

Either sub-lint defaults to `preserve` (no enforcement). A project
opts in by setting a non-`preserve` style.

## Sub-lint: `within_attribute`

Available styles:

- **`preserve`** (default) — no-op.
- **`alphabetical`** — every trait name in the list must be in
  ASCII-case-insensitive alphabetical order. Simple, predictable, no
  configuration required.
- **`prefix_then_alphabetical`** — a configurable list of trait
  names is the *fixed prefix*: those traits, when present, must
  appear first in the configured order. Anything else is sorted
  alphabetically after the prefix.

```toml
[derive_ordering.within_attribute]
style = "preserve"   # or "alphabetical" or "prefix_then_alphabetical"

# Used only for `prefix_then_alphabetical`. Traits listed here go
# first, in this order. Traits not listed are sorted alphabetically
# after.
prefix = [
  "Debug", "Default", "Clone", "Copy",
  "PartialEq", "Eq", "PartialOrd", "Ord",
  "Hash",
]

# When true, the lint matches by simple ident only. When false,
# it requires the full path (`std::fmt::Debug`) to match. Most
# projects want the simpler form.
match_by_ident = true
```

### Examples

Under `style = "alphabetical"`:

```rust
// Bad
#[derive(Debug, Clone, Copy)]

// Good
#[derive(Clone, Copy, Debug)]
```

Under `style = "prefix_then_alphabetical"` with the default `prefix`
above:

```rust
// Bad: PartialEq listed before Clone
#[derive(Debug, PartialEq, Clone)]

// Good: prefix order honored, Display alphabetised after the prefix
#[derive(Debug, Clone, PartialEq, Display)]
```

To recover parallel-disk-usage's "arbitrary" order verbatim, set the
prefix to its full enumeration of standard/comparison/Hash/derive_more
traits in the order parallel-disk-usage prefers.

## Sub-lint: `cross_attribute`

Available styles:

- **`preserve`** (default) — no-op.
- **`single_attribute`** — every derive belongs in one
  `#[derive(...)]` line per item. Multiple `#[derive(...)]`
  attributes on the same item are flagged and the suggested fix
  merges them (preserving the `within_attribute` order). Feature-
  gated `#[cfg_attr(.., derive(..))]` lines are left alone unless
  the cfg is the always-true case `cfg_attr(all(), ..)`, which is
  itself a smell handled separately.
- **`by_category`** — a configurable partition of trait names into
  ordered categories; each category occupies its own
  `#[derive(...)]` line. This is parallel-disk-usage's shape, made
  explicit:

  ```toml
  [derive_ordering.cross_attribute]
  style = "preserve"

  # Used by `by_category`. Each inner list is one category and
  # produces one `#[derive(...)]` line in that order. Traits not
  # listed in any category go in a final implicit "other" group
  # at the end, in `within_attribute` order.
  categories = [
    ["Debug", "Default", "Clone", "Copy"],
    ["PartialEq", "Eq", "PartialOrd", "Ord"],
    ["Hash"],
    # derive_more traits — list explicitly to keep the rule local.
    ["Display", "From", "Into",
     "Add", "AddAssign", "Sub", "SubAssign", "Sum",
     "Mul", "MulAssign", "Div", "DivAssign",
     "AsRef", "AsMut", "Deref", "DerefMut",
     "IntoIterator", "Constructor", "IsVariant"],
  ]

  # Whether feature-gated derives (`#[cfg_attr(.., derive(..))]`)
  # are required on their own line below the unconditional ones.
  feature_gated_below = true
  ```

### Examples

Under `style = "single_attribute"`:

```rust
// Bad: two #[derive(...)] lines
#[derive(Debug, Clone)]
#[derive(PartialEq, Eq)]
struct Foo;

// Good
#[derive(Debug, Clone, PartialEq, Eq)]
struct Foo;
```

Under `style = "by_category"` with the default categories:

```rust
// Bad: derive_more trait mixed with std traits
#[derive(Debug, Display, Clone, Copy)]
struct Foo(u64);

// Good
#[derive(Debug, Default, Clone, Copy)]
#[derive(Display)]
struct Foo(u64);
```

## Implementation notes

- `EarlyLintPass::check_item` to read attributes pre-expansion, since
  `#[derive]` is consumed before HIR.
- Within-attribute analysis is local: parse the comma-separated list
  and compare against the configured order.
- Cross-attribute analysis collects every `#[derive(...)]` and
  every `#[cfg_attr(.., derive(..))]` on the item, in source order.
- For `match_by_ident = false`, resolve each derive's path to a
  `DefId` (in `LateLintPass` via `cx.tcx.resolutions(..)`); the
  early-pass approach must fall back to ident matching only.
- Re-use a shared `name_of(meta)` helper to extract the trait ident
  from each derive entry. Account for paths like `serde::Deserialize`
  by using only the final segment when `match_by_ident = true`.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name prefixing (`perfectionist_*`)
  required for every registered lint.

## Interaction with other formatting

`cargo fmt` does *not* reorder derives. This lint is the only
mechanism for enforcing one. Suggested fixes are
`MachineApplicable` for `within_attribute` (string substitution) and
`MaybeIncorrect` for `cross_attribute` (merging or splitting attributes
can interact with `#[cfg_attr]` and any custom attribute macros sitting
between them).

## Severity

Warn for both sub-lints. Defaults to `preserve` so the rule is
zero-friction to adopt; opt in by configuring a style.
