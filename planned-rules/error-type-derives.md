# `error_type_derives`

**Sources:** parallel-disk-usage *Error Handling*; pacquet *Error
Handling*.

## Status

Bundles several sub-checks (`unused_error`, `missing_error`,
`unused_display`, `copyable_error`, `unconventional_error_name`).
Per
[one rule per file](../CLAUDE.md#one-rule-per-file-one-config-per-rule),
this file is expected to fan out into one planning file per sub-check
before implementation begins. The sub-checks have distinct trigger
predicates, disjoint configuration, and no shared diagnostic, so the
split is mechanical.

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

## Why restrict this?

This is a stylistic preference, not a correctness issue, with one
exception flagged below.

The Error sub-checks (`unused_error`, `copyable_error`,
`unconventional_error_name`): a derive list is read every time the
type is read, and reviewers infer the type's intent from it. A
`derive(Error)` on a type that's never used as an error, or that
the rest of the codebase agrees doesn't look like an error (`impl
Copy`, or named in a way the project reserves for non-errors),
creates a false signal — readers reason about the type as if it
might appear in a `Result`'s `Err` slot, and reviewers wave through
an unjustified `Error` impl on the assumption "the author had a
reason." Stripping the misleading derive (or, for the naming check,
renaming the type) shortens the time it takes a reader to classify
any given type as "value vs error."

The Display sub-check (`unused_display`): a `derive(Display)`
advertises a public-facing string representation. If nothing in the
crate consumes the impl (see the `unused_display` predicate below
for the consumer list), the advertisement is hollow — and a reader
trying to find "where do we render this type?" comes up empty.

The objectively-bad exception (`missing_error`): a type used as
`Result<_, T>`'s `Err` that does not implement `std::error::Error`
breaks `?`-interop with downstream `Box<dyn Error>` consumers,
prevents the value from participating in `source()` chains, and
blocks `anyhow::Error` / `eyre::Report` conversion. This is a real
defect, not a preference.

## What to lint

> [!IMPORTANT]
> **Lint name shape.** The `error_type_derives::` prefix used in the
> sub-check headings below is a documentation label grouping related
> checks under one banner. Per the
> [lint-name namespacing convention](./IMPLEMENTATION_CONVENTIONS.md#lint-name-namespacing),
> each sub-check is registered as its own flat tool lint
> `perfectionist::<sub_check_name>` (e.g.
> `perfectionist::copyable_error`). Suppression attributes use the
> flat form: `#[expect(perfectionist::copyable_error)]`.

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
almost always carry owned payload — a `String` message, a `PathBuf`,
a boxed `source` — that forbids `Copy`, so a `Copy` error is a
strong signal that the author wrote a plain data type and reflexively
reached for `Error` on the derive list. The motivating example was
`parallel_disk_usage::size::ParsedValue`, which derived `Copy + Error`
despite being the *successful* return type of
`Formatter::parse_value`.

The check is a heuristic, not an absolute defect: small unit-style
error enums (`enum ParseError { Empty, Negative }`) are legitimate
`Copy` errors. Suppress per-type with
`#[expect(perfectionist::copyable_error)]` when the heuristic misfires.

### `error_type_derives::unconventional_error_name`

Flag a type that derives or implements `std::error::Error` but whose
name does not match the project's error-naming convention. The default
pattern is the `Error` suffix, matching `std::io::Error`,
`serde_json::Error`, the thiserror documentation's examples, and the
parallel-disk-usage convention that motivated the rule. Configure
under the `["perfectionist::unconventional_error_name"]` table of the
consumer's `dylint.toml`, spelt per the
[lint-name namespacing convention](./IMPLEMENTATION_CONVENTIONS.md#lint-name-namespacing);
the `error_name_pattern` key accepts one of five forms — pick
exactly one (uncomment one line, leave the others commented):

```toml
["perfectionist::unconventional_error_name"]
# Default; equivalent to omitting the key.
error_name_pattern = { suffix = "Error" }
# Bare-string shorthand for `{ suffix = ... }`.
# error_name_pattern = "Error"
# List of suffixes; matches if the name ends with any.
# error_name_pattern = { suffix = ["Error", "Failure"] }
# Bare-list shorthand for `{ suffix = [...] }`.
# error_name_pattern = ["Error", "Failure"]
# Disable the sub-check (TOML has no `null` literal).
# error_name_pattern = false
```

Regex is intentionally *not* offered as a matcher form. See
[`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#parser-style)
for the project-wide rationale; for this rule specifically, suffix
matching covers the realistic configuration space without a regex
dependency.

Suppress per-type with
`#[expect(perfectionist::unconventional_error_name)]` when the
mismatch is intentional (e.g. a public type whose existing name
cannot be changed without a breaking release).

The check is one-directional: a type that *matches* the convention
but does not implement `Error` is not flagged here, because matching
the convention is exactly how a project declares "this is an error
type." The inverse pairing — a type whose name promises "error" but
which lacks an `Error` impl — is left to `missing_error` *when the
type is actually used as one* (it triggers on usage in `Result<_, E>`,
not on the name); a never-used type with an `Error`-shaped name is
intentionally not flagged by any sub-check, since silently-renaming
a never-used type is rarely what the author wanted.

## Examples

**Avoid:** `Error` derived but never used as one

```rust
#[derive(Debug, Display, Error)]
struct ConfigSummaryError(String);

fn main() {
    println!("{}", ConfigSummaryError("hi".into()));
}
```

**Prefer:** drop the `Error` derive; rename to drop the now-misleading `Error` suffix (the type isn't an error, even if it once was).

```rust
#[derive(Debug, Display)]
struct ConfigSummary(String);
```

**Avoid:** shape and name both signal this is not an error — `copyable_error` fires on `Copy + Error`, and `unconventional_error_name` fires on the missing `Error` suffix.

```rust
#[derive(Debug, Display, Clone, Copy, Error)]
pub enum ParsedValue {
    #[display("{value}   ")]
    Small { value: u16 },
    #[display("{coefficient:.1}{unit}")]
    Big { coefficient: f32, unit: char, scale: u64, exponent: usize },
}
```

**Prefer:**

```rust
#[derive(Debug, Display, Clone, Copy)]
pub enum ParsedValue { /* ... */ }
```

## Implementation notes

- This is a *whole-crate* lint. Implement it as a `LateLintPass` that
  records every `Result<_, E>` type, every `format_args!`-receiving span,
  and every `?` operator's error type during `check_crate`, then walks the
  recorded types and emits in a final pass via `check_crate_post`.
- Use `clippy_utils::ty::implements_trait` against `std::error::Error`
  and `std::fmt::Display` to confirm the type implements the trait
  (whether derived, hand-implemented, or otherwise).
- For the derive-keyed sub-checks (`unused_error`, `unused_display`),
  detection of the *derive* (versus a manual impl) requires inspecting
  the original attribute — preserve it in `check_item` before the
  implementation is desugared. `copyable_error` and
  `unconventional_error_name` side-step this by querying
  `implements_trait` directly (which succeeds for both derived and
  hand-rolled impls), since their predicate is "derives *or*
  implements `Error`."
- `copyable_error` and `unconventional_error_name` are *local* checks
  and need no crate-wide bookkeeping. The Error membership query is
  the global `implements_trait` check above; on top of that:
  `copyable_error` adds an `implements_trait` query against
  `std::marker::Copy`; `unconventional_error_name` strips generic
  parameters from the type's identifier (`MyError<T>` → `MyError`)
  and tests the result against the configured `error_name_pattern`.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Caveats

- A type may be used as an error only via downstream crates, in which case
  `unused_error` will false-positive. Allow `#[allow(...)]` on the type,
  and mention `pub` types in a softer category by default; in the
  consumer's `dylint.toml`, set:

  ```toml
  ["perfectionist::unused_error"]
  flag_pub_types = false
  ```

## Default state

Each sub-check has its own default state once it is split into a
sibling planning file per the Status section above. The expected
shape: `unused_display`, `unused_error`, `copyable_error`,
`unconventional_error_name`, and `missing_error` are all active
by default.
