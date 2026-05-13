# `error_type_derives`

**Sources:** parallel-disk-usage *Error Handling*; pacquet *Error
Handling*.

## Statement

> Use `derive_more` for error types. Only derive the traits that are
> actually used, and only derive `Error` on types that actually are
> errors.

Specifically:

- Derive `derive_more::Display` only when the type is actually displayed.
- Derive `derive_more::Error` only when the type is used as
  `std::error::Error` (the `Err` variant of a `Result`, or a `source`).
- A type that only needs formatting and not error handling should derive
  `Display` without `Error`.
- A type that does not look like an error — `impl Copy`, or named in a
  way the project reserves for non-errors — probably should not derive
  `Error` in the first place, even before usage is considered.

## What to lint

### `error_type_derives::unused_error`

Flag a type that derives `derive_more::Error` (or `thiserror::Error`) but
is never used as an error in the local crate: it never appears as the `E`
in a `Result<_, E>`, never as the return type of a `?`-using fn, and
never implements/forwards to another `Error` via `source`.

### `error_type_derives::missing_error`

Flag a type that is used as an error (appears as `E` in a `Result<_, E>`
return type) but derives neither `Error` nor implements `std::error::Error`
manually.

### `error_type_derives::unused_display`

Flag a type that derives `derive_more::Display` but is never formatted
(`format!`, `write!`, `println!`, `to_string`, `Display` super-trait
satisfaction, …) anywhere in the crate.

### `error_type_derives::copyable_error`

Flag a type that derives or implements `std::error::Error` *and* is
`Copy` (whether derived or hand-implemented). Production error types
almost always carry owned
payload — a `String` message, a `PathBuf`, a boxed `source` — that
forbids `Copy`, so a `Copy` error is a strong signal that the author
wrote a plain data type and reflexively reached for `Error` on the
derive list. The motivating example was
`parallel_disk_usage::size::ParsedValue`, which derived `Copy + Error`
despite being the *successful* return type of
`Formatter::parse_value`.

The check is a heuristic, not an absolute defect: small unit-style
error enums (`enum ParseError { Empty, Negative }`) are legitimate
`Copy` errors. Suppress with
`#[allow(perfectionist::error_type_derives::copyable_error)]` on the
type when the heuristic misfires.

### `error_type_derives::unconventional_error_name`

Flag a type that derives or implements `std::error::Error` but whose
name does not match the project's error-naming convention. The default
pattern is the `Error` suffix, matching `std::io::Error`,
`serde_json::Error`, the thiserror documentation's examples, and the
parallel-disk-usage convention that motivated the rule. Configure
under the `[error_type_derives]` table; the `error_name_pattern` key
accepts one of three forms:

```toml
[error_type_derives]
# Inline table tagged with the matcher kind. The default.
error_name_pattern = { suffix = "Error" }
# …or a regex matcher:
# error_name_pattern = { regex = ".*(Error|Failure)$" }
# `suffix` and `regex` are mutually exclusive; specifying both is a
# config error.

# Bare-string shorthand for the `suffix` form. The two lines below
# are equivalent.
error_name_pattern = "Error"
error_name_pattern = { suffix = "Error" }

# `false` disables the sub-check entirely (TOML has no `null`
# literal, so `false` is the off switch). Omitting the key applies
# the default `{ suffix = "Error" }` matcher.
error_name_pattern = false
```

The check is one-directional: a type that *matches* the convention
but does not implement `Error` is not flagged here, because matching
the convention is exactly how a project declares "this is an error
type". The inverse direction is covered by `missing_error` above,
which keys on usage rather than naming.

## Examples

```rust
// Bad: Error derived but never used as one
#[derive(Debug, Display, Error)]
struct ConfigSummary(String);

fn main() {
    println!("{}", ConfigSummary("hi".into()));
}

// Good
#[derive(Debug, Display)]
struct ConfigSummary(String);
```

```rust
// Bad: shape and name both signal this is not an error
//   — `copyable_error` fires on `Copy + Error`
//   — `unconventional_error_name` fires on the missing `Error` suffix
#[derive(Debug, Display, Clone, Copy, Error)]
pub enum ParsedValue {
    #[display("{value}   ")]
    Small { value: u16 },
    #[display("{coefficient:.1}{unit}")]
    Big { coefficient: f32, unit: char, scale: u64, exponent: usize },
}

// Good
#[derive(Debug, Display, Clone, Copy)]
pub enum ParsedValue { /* ... */ }
```

## Implementation notes

- This is a *whole-crate* lint. Implement it as a `LateLintPass` that
  records every `Result<_, E>` type, every `format_args!`-receiving span,
  and every `?` operator's error type during `check_crate`, then walks the
  recorded types and emits in a final pass via `check_crate_post`.
- Use `clippy_utils::ty::implements_trait` against `std::error::Error` and
  `std::fmt::Display` to confirm whether the derive landed.
- Detection of the *derive* (versus a manual impl) requires inspecting
  the original attribute — preserve it in `check_item` before the
  implementation is desugared.
- `copyable_error` and `unconventional_error_name` are *local* checks
  and need no crate-wide bookkeeping. Inspect the item's derive list
  and any `impl Error for T` block directly in `check_item`; for
  `copyable_error`, query `implements_trait` against both
  `std::marker::Copy` and `std::error::Error`.
- `unconventional_error_name`'s pattern check should match the type's
  identifier alone, with generic parameters stripped (`MyError<T>`
  matches an `Error` suffix on `MyError`).

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Caveats

- A type may be used as an error only via downstream crates, in which case
  this lint will false-positive. Allow `#[allow(...)]` on the type, and
  mention `pub` types in a softer category by default (configurable via
  `error_type_derives.flag_pub_types = false`).

## Severity

Warn for `unused_display`, `unused_error`, `copyable_error`, and
`unconventional_error_name` — all four are heuristics that admit
legitimate exceptions. Deny for `missing_error`, because using a
non-`Error` type as the error half of `Result` is almost always a bug.
