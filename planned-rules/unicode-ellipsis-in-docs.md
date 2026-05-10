# `unicode_ellipsis_in_docs`

**Source:** project convention (not present in either source document;
parallel to [`em-dash-prose`](./em-dash-prose.md), which targets U+2014).

## Statement

Forbid U+2026 HORIZONTAL ELLIPSIS (`…`) in doc comments. Prefer the
three-ASCII-dot form `...`.

Rationale:

- ASCII `...` survives every encoding round-trip, every terminal, every
  copy-paste, every `grep` invocation, and every `git diff` viewer
  without rendering as `?` or a tofu box.
- The visual difference between `…` and `...` is small enough that the
  Unicode form leaks into prose unintentionally (autocorrect, IDE
  smart-quote settings) without ever being a deliberate stylistic
  choice in technical writing.
- Diagnostics rendered in a non-UTF-8 terminal lose the ellipsis
  entirely.

## What to lint

For every `///` and `//!` doc-comment line, scan the rendered text for
U+2026 and emit a diagnostic at the precise span of the character.

Skip:

- Code spans (`` `...` ``) and code blocks (` ``` ... ``` ` or
  indented blocks). Code may legitimately contain `…` as part of an
  example string.
- Doc-test code (everything between `` ```rust `` … `` ``` ``). Same
  reasoning.

## Examples

```rust
// Bad
/// Walk the tree, collecting sizes…

// Good
/// Walk the tree, collecting sizes...
```

```rust
// Allowed: inside a code span
/// The format spec is `{value…}`.
```

## Implementation notes

- `EarlyLintPass::check_attribute`. For every `#[doc = "..."]` (the
  representation of `///` and `//!` after lexing), iterate the bytes
  of the doc string and locate U+2026 (encoded as `0xE2 0x80 0xA6`).
- Translate the byte offset back into a `Span` using
  `clippy_utils::source::position_before_rarrow`-style helpers, or
  more directly with `attr.value_str().unwrap()`'s span and an offset
  computed from the literal contents.
- For the code-span / code-block exclusion, use a tiny markdown
  scanner — track inside-backtick state and inside-fence state. The
  `pulldown_cmark` crate is a heavyweight dependency for a Dylint
  pass; a hand-written byte scanner is enough since we only need to
  recognise the three delimiters `` ` ``, `` ``` ``, and indented
  `    ` blocks.
- **Parser style.** Implement the markdown exclusion scanner as
  parser-combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
  `take_code_span` and `take_code_block` consume the excluded
  regions in one shot, leaving only the prose slice for the
  codepoint scan. Sharing this with the analogous step in
  [`intra-doc-links`](./intra-doc-links.md) and
  [`em-dash-prose`](./em-dash-prose.md) is the natural follow-up;
  factor the helper crate-internally rather than re-implementing
  per lint.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Autofix

Replace `…` with `...`. `Applicability::MachineApplicable`.

## Configuration

- `unicode_ellipsis_in_docs.also_flag = ["\u{22EF}", "\u{2025}"]` —
  empty by default. Lets a project extend the rule to midline
  ellipsis (`⋯`) or two-dot leader (`‥`).
- `unicode_ellipsis_in_docs.allow_in_code_spans = true` — defaults to
  `true`; set to `false` to enforce the rule even inside `` `...` ``.

## Severity

Warn.
