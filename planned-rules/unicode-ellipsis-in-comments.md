# `unicode_ellipsis_in_comments`

**Source:** project convention (parallel to
[`unicode-ellipsis-in-docs`](./unicode-ellipsis-in-docs.md), but applied
to non-doc comments).

## Statement

Forbid U+2026 HORIZONTAL ELLIPSIS (`…`) in regular `//` line comments
and `/* */` block comments. Prefer the three-ASCII-dot form `...`.

Same rationale as the docs variant: ASCII survives every tool, the
visual difference is too small to reward, and the character usually
arrives by accident from autocorrect.

## What to lint

For every comment that is **not** a doc comment (i.e., not `///`, not
`//!`, not `#[doc = "..."]`), scan the comment text for U+2026 and
emit a diagnostic at the character's span.

Comment varieties to cover:

- `// line comment`
- `/* block comment */` (single-line)
- Multi-line `/* … */` blocks (each line scanned independently for
  span purposes).

## Examples

```rust
// Bad
// TODO: handle the empty-tree case…

// Good
// TODO: handle the empty-tree case...
```

```rust
// Bad
/* fall through into the slow path … */

// Good
/* fall through into the slow path ... */
```

## Implementation notes

- Doc comments and regular comments are not exposed identically by
  rustc. `///` and `//!` arrive as `#[doc = "..."]` attributes, but
  plain `//` and `/* */` comments are stripped before HIR. Read them
  via `tcx.sess.source_map()`'s `SourceFile::src` and re-tokenize with
  `rustc_lexer::tokenize`, then filter to the
  `rustc_lexer::TokenKind::LineComment { doc_style: None, .. }` and
  `BlockComment { doc_style: None, .. }` variants.
- For each comment token, scan its byte range for the UTF-8 sequence
  `0xE2 0x80 0xA6` and emit at the precise sub-span.
- Run from `LateLintPass::check_crate` (or `EarlyLintPass::check_crate`
  with the same source-map access). One pass per crate, not per item.

## Autofix

Replace `…` with `...`. `Applicability::MachineApplicable`.

## Configuration

- `unicode_ellipsis_in_comments.also_flag` — same shape as the docs
  rule.
- `unicode_ellipsis_in_comments.scope = ["line", "block"]` — restrict
  to one of the two comment styles if a project wants finer control.

## Severity

Warn.

## Why this is a separate lint from `unicode_ellipsis_in_docs`

The two rules detect the same character in different contexts and a
project might reasonably enable one without the other. Doc comments
become rendered HTML on docs.rs, where Unicode ellipses *do* render
correctly; some projects therefore tolerate `…` in docs but want it
banned from internal `// FIXME …` comments that land in `grep` output.
Splitting the rules lets each project decide.
