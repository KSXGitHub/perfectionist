# `prefer_derive_more_over_thiserror`

**Source:** project convention. Sibling to
[`prefer-derive-more`](./prefer-derive-more.md), which catches
hand-written `impl` blocks. This rule catches the *other* common
alternative: error types derived with [`thiserror`](https://docs.rs/thiserror)
instead of `derive_more`.

## Statement

Blanket ban on `thiserror`. The catalogue picks `derive_more` for
error formatting and source-chaining; mixing in `thiserror`
fragments the attribute vocabulary across the codebase and adds a
second derive crate that has no functional capability `derive_more`
lacks.

The rule emits a help-only diagnostic on every site that uses
`thiserror`. There is **no autofix**: the migration involves a
mix of derive-list edits, format-string positional translation
(`thiserror`'s `{0}` ↔ `derive_more`'s `{_0}`), attribute renames
(`#[error(...)]` ↔ `#[display(...)]`), and edge cases
(`#[error(transparent)]`, `#[backtrace]`) whose mechanical
rewrite is too risky to apply without review. The diagnostic
points at the offending site and suggests `#[derive(Display, Error)]`
as the target shape; the contributor performs the migration by
hand.

## What to lint

Three flavours of `thiserror` use trigger the rule:

1. **`#[derive(thiserror::Error)]`** (or `#[derive(Error)]` with
   `use thiserror::Error;` in scope on the same item). Diagnostic
   span is the offending derive entry.
2. **`#[error(...)]` attributes** that come from `thiserror`'s
   attribute namespace. Detected by sibling `#[derive(Error)]`
   resolving to `thiserror::Error` on the enclosing item.
3. **`use thiserror::*` / `use thiserror::Error`** (or any
   re-export-style import that brings `thiserror` into scope).
   Diagnostic span is the `use` statement.

For each match, emit:

> error type derived through `thiserror`; this catalogue prefers
> `derive_more::{Display, Error}`. Replace the derive list and
> migrate the `#[error(...)]` attributes to `#[display(...)]`.

The diagnostic carries no `Suggestion` and no `Applicability`.

## Examples

```rust
// Bad
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MyError {
    #[error("missing field {0}")]
    MissingField(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

// Target shape (apply by hand)
use derive_more::{Display, Error, From};

#[derive(Debug, Display, Error, From)]
pub enum MyError {
    #[display("missing field {_0}")]
    #[from(ignore)]
    MissingField(String),
    #[display("io: {_0}")]
    Io(std::io::Error),
}
```

The migration involves several mechanical steps:

- Swap the derive list: `Error` → `Display, Error`.
- Add `From` to the derive list when `#[from]` field annotations
  are present (derive_more's `From` is a separate derive rather
  than a per-field attribute).
- Rename `#[error(...)]` → `#[display(...)]`.
- Translate positional placeholders: `{0}` / `{1}` / … →
  `{_0}` / `{_1}` / ….
- Resolve special cases manually: `#[error(transparent)]` becomes
  `#[display(forward)]` plus appropriate `#[error(forward)]` on
  the enum; `#[backtrace]` does not have a direct equivalent and
  needs a manual `Error::provide` impl.

The rule does not attempt to perform any of these rewrites; the
diagnostic is informational.

## Configuration

```toml
[prefer_derive_more_over_thiserror]
# Recognised thiserror paths. Defaults cover the canonical crate;
# add forks here if a project re-exports under a custom name.
thiserror_paths = ["thiserror::Error"]
```

## Implementation notes

- `LateLintPass`. Two halves:
  1. **Identify thiserror-derived items.** For every
     `ItemKind::Struct` or `ItemKind::Enum`, walk the
     `#[derive(...)]` attributes. Resolve each derive's path and
     match against `thiserror_paths`. Re-exports (`pub use
     thiserror::Error;`) are caught via `DefId` resolution.
  2. **Identify the `use thiserror::...` import.** Walk
     `ItemKind::Use` and match the path's first segment against
     the configured `thiserror_paths`' crate name (`thiserror`).
- **No autofix.** The lint emits a `Span` plus the help text
  above with no `Suggestion` and no `Applicability::*`. `cargo
  clippy --fix` cannot rewrite the offending site.

### Difficulty

**Easy.** The rule is detection-only — no template parsing, no
cross-impl coordination, no semantic equivalence proof. The full
migration *is* hard (see the bullet list above), but that
complexity sits with the contributor performing the migration by
hand, not in the lint itself.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Default state

Active by default.

## Interaction with sibling lints

- [`prefer-derive-more`](./prefer-derive-more.md) catches
  hand-written `impl Display` / `impl Error` blocks and suggests
  the corresponding `derive_more` derives.
- [`error-type-derives`](./error-type-derives.md) checks that
  `Display` / `Error` are derived only when actually used.
- This rule (`prefer_derive_more_over_thiserror`) flags
  `thiserror`-derived sites and steers them toward `derive_more`.

A type may hit several of these in succession during a migration:
flag the `thiserror` derive, contributor migrates by hand to
`derive_more::{Display, Error}`, then `error-type-derives` checks
the result is actually used as an error in the local crate.
