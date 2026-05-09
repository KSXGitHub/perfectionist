# `em_dash_prose`

**Source:** parallel-disk-usage *Writing Style*.

## Statement

> Avoid mid-sentence breaks introduced by em dashes or long parenthetical
> clauses. Em dashes are a reliable symptom of loose phrasing; when one
> appears, restructure the surrounding sentence so each clause stands on
> its own.

## What to lint

Flag the U+2014 EM DASH (`—`) and the U+2013 EN DASH (`–`) when used as
sentence punctuation in:

- `///` and `//!` doc comments (likely the highest-value target).
- `//` regular comments (configurable, on by default).
- String literals passed to user-facing macros: `println!`, `eprintln!`,
  `format!`, `write!`, `writeln!`, `print!`, `eprint!`, `panic!`,
  `unimplemented!`, `todo!`, `unreachable!`, and the `log::*!` family.

Do *not* flag em dashes when they appear:

- Inside a code-block fence in a doc comment (` ``` ... ``` ` /
  `` `...` ``).
- In a string literal that is clearly data and not user-facing prose
  (heuristic: contains no word characters around the dash, e.g., a Unicode
  test corpus).
- In test files (`#[cfg(test)]`), which often embed expected output that
  legitimately contains an em dash.

## Examples

```rust
// Bad
/// Walks the tree — including hidden directories — and returns the
/// total size.

// Good
/// Walks the tree, including hidden directories, and returns the total
/// size.

// Bad
println!("Skipping path — {path:?}");

// Good
println!("Skipping path: {path:?}");
```

## Implementation notes

- `EarlyLintPass::check_attribute` for doc comments and
  `check_expr` for macro invocations. Macro detection: walk
  `ExprKind::Macro` (post-expansion, look for `Span::from_expansion`
  and the macro `DefId`'s diagnostic name).
- For doc comments, strip code spans and code blocks before scanning.
- The autofix is *not* mechanical because the surrounding sentence often
  needs restructuring. Emit a help-only suggestion that points to the
  dash and recommends restructuring.

## Configuration

- `em_dash_prose.targets` — array of `"doc"`, `"comment"`, `"macro"`.
- `em_dash_prose.flag_en_dash` — defaults to `true`.
- `em_dash_prose.allow_in_tests` — defaults to `true`.

## Severity

Warn.
