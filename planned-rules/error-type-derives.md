# `error_type_derives`

**Sources:** parallel-disk-usage *Error Handling*; pacquet *Error
Handling*.

## Statement

> Use `derive_more` for error types. Only derive the traits that are
> actually used.

Specifically:

- Derive `derive_more::Display` only when the type is actually displayed.
- Derive `derive_more::Error` only when the type is used as
  `std::error::Error` (the `Err` variant of a `Result`, or a `source`).
- A type that only needs formatting and not error handling should derive
  `Display` without `Error`.

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

Warn for `unused_display` and `unused_error`. Deny for `missing_error`,
because using a non-`Error` type as the error half of `Result` is almost
always a bug.
