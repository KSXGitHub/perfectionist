# `prefer_derive_more_over_thiserror`

**Source:** project convention. Sibling to
[`prefer-derive-more`](./prefer-derive-more.md), which catches
hand-written `impl` blocks. This rule catches the *other* common
alternative: error types derived with [`thiserror`](https://docs.rs/thiserror)
instead of `derive_more`.

## Statement

Prefer `derive_more::{Display, Error}` over any form of `thiserror`.
The two crates have largely overlapping feature sets for error
types; this catalogue picks `derive_more` because the project
already depends on it (or should — see
[`error-type-derives`](./error-type-derives.md)) for non-error
formatting and constructor derives, and because keeping one
attribute vocabulary across the codebase reduces context-switching.

The rule flags every site that uses `thiserror` and suggests the
corresponding `derive_more` form.

## What to lint

Three flavours of `thiserror` use are recognised:

1. **`#[derive(thiserror::Error)]` (or `#[derive(Error)]` with a
   `use thiserror::Error;` in scope).** Suggest replacing with
   `#[derive(derive_more::Display, derive_more::Error)]`.
   `thiserror::Error` provides both `Display` and `Error`, so the
   suggestion adds both `derive_more` derives at once.
2. **`#[error("template", args...)]` attributes.** Suggest
   replacing with `#[display("template", args...)]`. Format
   strings need positional translation: `thiserror`'s `{0}`,
   `{1}`, … reference fields and become `derive_more`'s
   `{_0}`, `{_1}`, … Named field references (`{name}`) carry over
   verbatim.
3. **`#[error(transparent)]` attribute.** Suggest replacing with
   the equivalent `derive_more` shape: `#[display(forward)]` on
   the variant *and* `#[error(forward)]` on the enum if the source
   should also be forwarded. This sub-case is the trickiest and
   the autofix is `MaybeIncorrect`.

The other `thiserror`-specific attributes carry over with no
syntax change: `#[from]`, `#[source]`, and `#[backtrace]` mean the
same thing to `derive_more` (modulo small differences for
`#[backtrace]` — `derive_more` uses the `Error::provide` API where
`thiserror` has its own backtrace handling). The lint flags the
`use thiserror::*` import and the derive but does not separately
flag these annotations.

## Examples

### Simple variant with format string

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

// Good
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

Note the format-string positional translation (`{0}` → `{_0}`)
and the lifting of `#[from]` from a field annotation to a
top-level derive (`derive_more`'s `From` is a separate derive
rather than a field attribute).

### Transparent forwarding

```rust
// Bad
#[derive(Debug, thiserror::Error)]
pub enum Wrapper {
    #[error(transparent)]
    Inner(#[from] InnerError),
}

// Good
#[derive(Debug, derive_more::Display, derive_more::Error,
         derive_more::From)]
pub enum Wrapper {
    #[display(forward)]
    Inner(InnerError),
}
```

`#[display(forward)]` delegates `Display` to the inner type;
`derive_more::Error` automatically forwards `source()` for tuple
variants holding a single `Error`-implementing field.

### Pure rename

```rust
// Bad
use thiserror::Error;
struct ParseError(String);

#[derive(Debug, Error)]
#[error("parse error: {0}")]
struct ParseError(String);

// Good
use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
#[display("parse error: {_0}")]
struct ParseError(String);
```

## Configuration

```toml
[prefer_derive_more_over_thiserror]
# The lint has no style switch — the policy is unidirectional.
# Set `enabled = false` to disable entirely.
enabled = true

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
  2. **For each match, inspect the type's other attributes** to
     determine which `thiserror`-specific shapes are in use and
     compose the suggested rewrite.
- Format-string translation:
  - Parse the `#[error("template")]` string.
  - Walk for `{N}` (positional, where `N` is a non-negative integer).
  - Replace each with `{_N}`. Named references and `{}` (auto-
    indexed positional, less common in `thiserror`) are left
    alone — `derive_more` supports the same syntax for those.
  - Reuse the format-string scanner from
    [`derive-more-inlined-args`](./derive-more-inlined-args.md);
    factor the helper crate-internally.
- `#[error(transparent)]` translation: the `transparent` keyword
  doesn't translate one-to-one to a `derive_more` attribute. The
  replacement is `#[display(forward)]` on the variant; the
  `Error::source` forwarding is implicit when the variant holds a
  single `Error`-implementing field that derive_more's `Error`
  derive can pick up automatically. Bail to help-only suggestion
  if the variant has multiple fields or carries other annotations
  the lint can't disentangle.
- `#[from]` field-attribute translation: thiserror's `#[from]` on
  a field is structurally the same as derive_more's `#[from]` on
  a variant. The suggested rewrite adds `derive_more::From` to
  the derive list and keeps the field annotation as-is.
- `#[backtrace]` translation: derive_more does not have a direct
  equivalent. Bail to help-only suggestion noting that backtrace
  capture must be re-implemented manually via
  `Error::provide`. **The autofix never touches a type that uses
  `#[backtrace]`.**
- **Parser style.** Implement the format-string scanner as
  parser-combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md).
  Reuse the placeholder/literal-text helpers from
  [`derive-more-inlined-args`](./derive-more-inlined-args.md).

### Difficulty

**Medium.** Comparable to
[`prefer-derive-more`](./prefer-derive-more.md)'s `display`
sub-lint — same family of concern (rewriting one derive vocabulary
into another), but easier in practice because both vocabularies
share most attribute names and the format-string differences are
mechanical.

The high-risk sub-cases (`#[error(transparent)]`,
`#[backtrace]`) are walled off as `MaybeIncorrect` or help-only
suggestions; the simple `#[error("...", ..)]`-only cases get a
clean `MachineApplicable` rewrite.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Severity

Warn. The autofix is `MachineApplicable` for the common case
(plain `#[derive(thiserror::Error)]` with `#[error("...")]`
strings whose only positional placeholders are `{N}`-form) and
`MaybeIncorrect` for everything else.

## Interaction with sibling lints

- [`prefer-derive-more`](./prefer-derive-more.md) catches
  hand-written `impl Display` / `impl Error` blocks and suggests
  the corresponding `derive_more` derives.
- [`error-type-derives`](./error-type-derives.md) checks that
  `Display` / `Error` are derived only when actually used.
- This rule (`prefer_derive_more_over_thiserror`) catches
  `thiserror`-derived sites and steers them toward `derive_more`.

A type may hit several of these in succession: `thiserror::Error`
becomes `derive_more::{Display, Error}`, then
`error-type-derives` checks the result is actually used as an
error in the local crate.
