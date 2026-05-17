# `em_dash_prose`

**Source:** parallel-disk-usage *Writing Style*.

## Statement

> Avoid mid-sentence breaks introduced by em dashes or long parenthetical
> clauses. Em dashes are a reliable symptom of loose phrasing; when one
> appears, restructure the surrounding sentence so each clause stands on
> its own.

## Why restrict this?

This is a stylistic preference, not a correctness issue. The em dash
is a hallmark of AI-generated text. Modern code-writing
assistants reach for it whenever they want to glue two clauses together
without committing to a structural relationship. The result reads as
loose, conversational prose — exactly the register technical
documentation should not adopt. A human author writing carefully will
almost never pick `—` over a comma, a semicolon, a colon, or a
sentence break, because each of those alternatives makes the
relationship between clauses explicit. The em dash hides it.

This lint exists to catch that signal early and force the author (or
the AI) to do the structural work the dash was eliding.

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
/// Walks the tree, including hidden directories. Returns the total
/// size.

// Bad
println!("Skipping path — {path:?}");

// Good
println!("Skipping path: {path:?}");
```

## Diagnostic message

The diagnostic must instruct both human authors *and* automated
assistants explicitly. A drive-by replacement of `—` with `,`, `;`, or
`:` is **not** the requested fix; the sentence almost always needs to
be restructured into shorter, complete sentences. The diagnostic
should say so directly.

The implementation should emit text along these lines:

> **error:** em dash (`—`) found in prose
>
> **note:** em dashes are characteristic of AI-generated text and
> indicate that the surrounding clauses have not been given a
> deliberate structural relationship. Formal technical prose should
> not contain em dashes.
>
> **help:** do not simply replace `—` with `,`, `;`, `:`, `(`, `)`,
> or `--`. Restructure the sentence so each clause stands on its own.
> Split into separate sentences, or pick a punctuation mark that
> makes the relationship between clauses explicit (a colon for
> elaboration, a semicolon for parallel statements, a comma for a
> dependent clause). The result should read as deliberate, formal
> writing.
>
> **AI assistants:** when you encounter this diagnostic in your own
> output, treat it as a request to *rewrite* the surrounding
> paragraph, not a request to substitute a different punctuation
> character. The presence of an em dash is evidence that the
> sentence's structure was elided; restoring that structure is the
> fix.

The exact wording is implementation detail, but the message must
convey: (1) the em dash is itself an AI tell, (2) substitution is not
acceptable, (3) restructuring into formal prose is the required
action.

## Implementation notes

- `EarlyLintPass::check_attribute` for doc comments and
  `check_expr` for macro invocations. Macro detection: walk
  `ExprKind::Macro` (post-expansion, look for `Span::from_expansion`
  and the macro `DefId`'s diagnostic name).
- For doc comments, strip code spans and code blocks before scanning.
- Use the shared markdown scanner (Tier B — code-region mask) per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#markdown-parsing)
  for the code-span / code-block exclusion.
- **No autofix is offered, ever.** The lint is deliberately
  fix-resistant: a tool-generated `—` → `,` substitution would mask
  the symptom while leaving the underlying loose phrasing in place,
  which is the opposite of what the rule is for. The diagnostic emits
  the dash's span as a `Span` only — no `Suggestion`, no
  `Applicability::*` annotation — so neither `cargo clippy --fix`
  nor any third-party tooling can mechanically rewrite it.
- The diagnostic includes the `--note` and `--help` text described
  above as static strings.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Configuration

- `em_dash_prose.targets` — array of `"doc"`, `"comment"`, `"macro"`.
- `em_dash_prose.flag_en_dash` — defaults to `true`.
- `em_dash_prose.allow_in_tests` — defaults to `true`.
- `em_dash_prose.message` — optional override for the help text.
  Useful for projects that want to localise the diagnostic or expand
  it with a link to an internal style guide. The default message
  above is used when this is unset.

## Default state

Active by default.
