# `derive_ordering`

**Source:** parallel-disk-usage *Derive Macro Ordering*. (pacquet does not
restate the rule, but does not contradict it.)

## Statement

When deriving multiple traits, split the derives across multiple
`#[derive(...)]` attributes in this order:

1. Standard traits: `Debug`, `Default`, `Clone`, `Copy`.
2. Comparison traits: `PartialEq`, `Eq`, `PartialOrd`, `Ord`.
3. `Hash`.
4. `derive_more` traits (`Display`, `From`, `Into`, `Add`, `AddAssign`,
   `Sub`, `SubAssign`, `Sum`, `Mul`, `MulAssign`, `Div`, `DivAssign`,
   `AsRef`, `AsMut`, `Deref`, `DerefMut`, `IntoIterator`, `Constructor`,
   `IsVariant`, etc.).
5. Feature-gated derives, on a separate `#[cfg_attr(feature = "...",
   derive(...))]` line.

## What to lint

Two related diagnostics:

### `derive_ordering::cross_attribute`

Flag the case where two consecutive `#[derive(...)]` attributes mix
categories backwards — for example, a comparison trait listed *before* a
standard trait, or a `derive_more` trait alongside `Debug` in the same
attribute.

### `derive_ordering::within_attribute`

Within a single `#[derive(A, B, C, …)]`, flag traits listed out of
intra-category order (`Debug` after `Default`; `Eq` before `PartialEq`).

## Examples

```rust
// Good
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[derive(From, Into, Add, AddAssign, Sub, SubAssign, Sum)]
#[cfg_attr(feature = "json", derive(Deserialize, Serialize))]
pub struct Bytes(u64);

// Bad: derive_more trait mixed with std traits
#[derive(Debug, Display, Clone, Copy)]
pub struct Bytes(u64);
```

## Implementation notes

- `EarlyLintPass::check_item` to read attributes pre-expansion, since
  `#[derive]` is consumed before HIR.
- Maintain a static list of standard, comparison, hash, and `derive_more`
  trait paths. The "is this from `derive_more`?" check needs path
  resolution; in early-pass we can match on the literal ident plus an
  optional bypass via `dylint.toml` for projects that re-export their
  own derives.
- `serde::{Deserialize, Serialize}` is recognised in the feature-gated
  category by default.

## Configuration

- `derive_ordering.standard_traits`
- `derive_ordering.comparison_traits`
- `derive_ordering.hash_traits`
- `derive_ordering.derive_more_traits`
- `derive_ordering.feature_gated_traits`

## Severity

Warn.
