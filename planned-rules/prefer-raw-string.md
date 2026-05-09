# `prefer_raw_string`

**Source:** project convention.

## Statement

When a string literal contains escape sequences for printable
characters (`\"`, `\\`, `\'`), prefer the raw-string form
(`r"..."` or `r#"..."#`) which avoids the escapes entirely.

Whitespace and control-character escapes (`\n`, `\t`, `\r`, `\0`)
and Unicode escapes (`\x..`, `\u{..}`) are exempt — they cannot be
expressed in a raw string and the regular form is the only choice.
A string that mixes escapable and inexpressible escapes is also
left alone; conversion would force the author to split the string
or fall back to `concat!`, which loses more than it gains.

The rule trades one noise source (interior backslash escapes) for a
slightly more elaborate string syntax. The benefit is highest in
strings full of file paths, regex patterns, JSON snippets, or
embedded source code — all of which would otherwise be a sea of
`\\` and `\"`.

## What to lint

For every string literal (`ExprKind::Lit` of `LitKind::Str`):

1. Skip if the literal is already a raw string (its source spelling
   begins with `r"` or `r#`).
2. Walk the source spelling between the surrounding quotes and
   classify each escape sequence:
   - **Eliminable** (would disappear in raw form): `\"`, `\\`,
     `\'`. Configurable via `escapes_eligible`.
   - **Required non-raw** (cannot exist in a raw string): `\n`,
     `\t`, `\r`, `\0`, `\x..`, `\u{..}`, and any other escape that
     produces a non-printable or whitespace codepoint.
3. If any required-non-raw escape is present, bail.
4. If the count of eliminable escapes is at least
   `min_escapes_to_trigger` (default 1), emit a diagnostic at the
   string literal's span and suggest the raw-string form.

## Examples

```rust
// Bad: escaped quotes
let json = "{\"name\":\"foo\"}";
// Good
let json = r#"{"name":"foo"}"#;

// Bad: escaped backslashes (Windows path)
let path = "C:\\Users\\foo\\bar";
// Good
let path = r"C:\Users\foo\bar";

// Bad: escaped quote in regex
let pattern = "say \"hello\"";
// Good
let pattern = r#"say "hello""#;

// Not flagged: contains a required non-raw escape
let label = "name:\tvalue\n";

// Not flagged: mixed escapable + non-raw
let mixed = "She said \"hi\" then\nleft.";
// (raw form can't carry the \n; leave the author to decide whether
// to split, restructure, or accept the escapes.)

// Not flagged: already raw
let template = r#"<div class="x">"#;
```

## Choosing the hash count

The autofix picks the *smallest* number of `#` characters such
that the closing `"#...#` sequence does not appear inside the
string. The algorithm:

1. Start with `n = 0` (i.e., `r"..."`).
2. Scan the string for `"` followed by `n` consecutive `#`
   characters. If that pattern is not present, `n` is fine.
3. Otherwise, increment `n` and retry.

In practice `n` is 0 or 1 for almost every string; longer hash
runs are needed only when the string itself contains literal
`"#...` sequences (e.g., embedded raw-string snippets).

## Configuration

```toml
[prefer_raw_string]
# Set to false to disable the rule entirely.
enabled = true

# Minimum number of eliminable escapes a string must contain before
# the lint fires. Default 1 catches every escapable string; set to
# 2 to skip single-escape literals where the raw form is arguably
# noisier than the original.
min_escapes_to_trigger = 1

# Escape sequences considered eliminable by switching to raw form.
# Default covers the three cases that a raw string can express
# verbatim without any escape.
escapes_eligible = ["\\\"", "\\\\", "\\'"]
```

## Implementation notes

- `LateLintPass::check_expr` on `ExprKind::Lit` of `LitKind::Str`.
  Use the lit's *source spelling* via
  `cx.sess().source_map().span_to_snippet(lit.span)`, not the
  decoded value — the rule operates on the syntactic form, not
  the semantic content.
- The bare check ("does the source spelling start with `r`?") is
  enough to skip already-raw strings; no need to inspect
  `LitKind::Str(_, StrStyle::Raw(_))` separately, but the latter
  is also fine.
- **Parser style.** Implement the escape scanner as parser-
  combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
  `take_literal_char` (any non-backslash byte),
  `take_escape_eliminable` (`\"`, `\\`, `\'`),
  `take_escape_non_raw` (`\n`, `\t`, `\r`, `\0`, `\x..`, `\u{..}`).
  The scanner returns a tuple `(eliminable_count,
  has_non_raw_escape)`; the lint dispatches on the result.
- The autofix:
  - Strip the outer `"..."`.
  - Compute the minimal hash count `n` per the algorithm above.
  - Emit `r#..."{contents}"#...` with `n` hash marks on each side.
  - `Applicability::MachineApplicable` because the rewrite is a
    pure syntactic substitution that produces an identical string
    value.

### Difficulty

**Easy.** Pure string-syntax analysis with a small fixed grammar
and no cross-item coordination. The hash-count selection is the
only non-trivial step and degenerates to constant-time work for
real-world inputs.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Severity

Warn. The autofix is `MachineApplicable` whenever the lint fires.

## Interaction with `clippy::needless_raw_strings` and `clippy::needless_raw_string_hashes`

The clippy lints fire in the *opposite* direction — they push
authors *away* from raw strings when no escape benefit exists, and
toward fewer hash marks when the count is gratuitously high. The
two perspectives compose cleanly:

- A regular string with no eliminable escapes: neither rule
  fires.
- A regular string with eliminable escapes: this rule fires,
  suggests raw form. After the rewrite, the result has the
  smallest viable hash count, so `clippy::needless_raw_string_hashes`
  also stays silent.
- A raw string with no escape pressure: `clippy::needless_raw_strings`
  fires, suggests stripping the `r`. This rule does not contradict.
- A raw string with too many `#`s: `clippy::needless_raw_string_hashes`
  fires. This rule does not contradict.

Enable both clippy lints alongside this one for a consistent
back-and-forth that always leaves the codebase in the minimal-
escape, minimal-hash form.
