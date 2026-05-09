# `serde_source_types`

**Source:** pacquet *Serde `Cow<'de, str>` vs `String` source types*.

## Statement

When a type uses `#[serde(from = "...")]` or `#[serde(try_from = "...")]`
to delegate deserialization, choose the source according to whether the
deserialized value retains the string:

- **Never** use `&'de str`. Borrowed deserialization rejects every input
  that requires escape-sequence decoding (e.g., JSON's `"a"`).
- Prefer `Cow<'de, str>` when the conversion discards the string or splits
  it.
- Prefer `String` when the entire input is moved into the result.

## What to lint

### `serde_source_types::borrowed_str`

Flag any `#[serde(from = "&'de str")]`,
`#[serde(try_from = "&'de str")]`, `#[serde(from = "&str")]`, or
`#[serde(try_from = "&str")]` attribute. Suggest replacing the source
type with `Cow<'de, str>` (default) or `String` (with help text noting
the alternative).

This sub-lint can fire on attributes alone — no whole-program analysis is
needed.

### `serde_source_types::cow_or_string` (advisory)

Optional, off by default. For a type using
`#[serde(from = "Cow<'de, str>")]`, inspect the `From<Cow<'a, str>>`
impl: if the body unconditionally calls `into_owned()` and stores the
result, suggest `String` as the source. If the body uses the string only
through borrowed APIs (`.as_ref()`, `.parse()`, slice indexing) and
discards the rest, leave `Cow<'de, str>` alone.

The advisory form is heuristic and easy to false-positive on. Ship it
gated behind `serde_source_types.advisory = true` in `dylint.toml`.

## Examples

```rust
// Bad
#[derive(serde::Deserialize)]
#[serde(try_from = "&'de str")]
struct Port(u16);

// Good (retains nothing)
#[derive(serde::Deserialize)]
#[serde(try_from = "Cow<'de, str>")]
struct Port(u16);

// Good (retains the entire input)
#[derive(serde::Deserialize)]
#[serde(try_from = "String")]
struct PackageName(String);
```

## Implementation notes

- `EarlyLintPass::check_item` reading the raw attribute tokens. Serde
  `from` / `try_from` arguments are *string literals* containing a Rust
  type, so the lint must parse the literal's contents.
- A small parser that recognises `&str`, `&'<lifetime> str`, the same
  with leading `core::` / `std::` paths, and emits accordingly is
  sufficient. `syn` is *not* available in a Dylint pass; instead, lex the
  string with `rustc_lexer` or do a minimal hand-written check
  (`trimmed.starts_with('&') && ends_with("str")`).

## Severity

Deny for `borrowed_str` (it produces silently broken parsers). Warn for
the advisory `cow_or_string` sub-lint.
