# `non_exhaustive_error`

**Source:** parallel-disk-usage *Error Handling* (the snippet shows
`#[non_exhaustive]` on every error enum); pacquet *Error Handling* uses the
same pattern in its example.

## Statement

Public error enums should carry `#[non_exhaustive]` so that adding a
variant in a future version is not a SemVer break for downstream pattern
matches.

## What to lint

For every `pub` (or `pub(crate)` in a library crate) `enum` whose name ends
in `Error` *or* which derives `derive_more::Error` / `thiserror::Error` /
implements `std::error::Error`, require a `#[non_exhaustive]` attribute on
the enum.

The same rule applies to `pub` `struct`s with the same heuristic, but
only when they are sum-like (single field is itself an enum) — flag at
warn level only, since structs benefit less from `non_exhaustive`.

## Examples

```rust
// Bad
#[derive(Debug, Display, Error)]
pub enum RuntimeError {
    SerializationFailure(serde_json::Error),
}

// Good
#[derive(Debug, Display, Error)]
#[non_exhaustive]
pub enum RuntimeError {
    SerializationFailure(serde_json::Error),
}
```

## Implementation notes

- `LateLintPass::check_item` on `ItemKind::Enum` with `Visibility::Public`
  (or pub-crate, configurable).
- "Looks like an error" predicate:
  - The ident ends with `Error`, **or**
  - The type implements `std::error::Error` (via
    `clippy_utils::ty::implements_trait`).
- Look for `#[non_exhaustive]` on the item's attribute list.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Configuration

- `non_exhaustive_error.require_for = ["pub"]` — `"pub_crate"` and
  `"all"` also accepted.
- `non_exhaustive_error.suffixes = ["Error"]` — extend with project
  conventions like `Failure`.

## Severity

Warn.
