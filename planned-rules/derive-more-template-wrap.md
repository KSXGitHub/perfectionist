# `derive_more_template_wrap`

**Source:** project convention. Sibling to
[`format-macro-wrap`](./format-macro-wrap.md) and
[`print-macro-split`](./print-macro-split.md). The same
"too-wide source line carrying a format template" problem appears
in three places; this rule covers the third — `derive_more`'s
attribute-form templates.

## Statement

A `derive_more` `#[display(...)]` or `#[debug(...)]` attribute
whose source line exceeds the configured width is hard to read
and produces a noisy diff:

```rust
#[display("error: The error was caused by {_0}\nhint: Run {_1} to solve the problem")]
struct UserMessage(String, String);
```

The attribute is consumed by a derive macro, so splitting into
multiple attributes is not viable — `derive_more` reads exactly
one `#[display(...)]` per item. The only applicable rewrite is
folding the template across multiple source lines with
`\<newline>` continuations:

```rust
#[display(
    "error: The error was caused by {_0}\n\
    hint: Run {_1} to solve the problem"
)]
struct UserMessage(String, String);
```

The rewrite produces a template byte-equivalent to the original;
the derive's behaviour is unchanged.

## What to lint

For every recognised attribute on a struct, enum, or variant:

1. Resolve the attribute path. Skip if it isn't in
   `attribute_paths`.
2. Locate the format template (the first string-literal positional
   argument of the attribute's argument list).
3. Skip if the template is not a string literal (it's a
   `concat!`-style expression or a path).
4. Compute the source-line span of the entire attribute. If its
   width is `≤ max_line_width`, skip — the attribute is short
   enough to leave alone. Width is unicode display width, the
   same metric as
   [`prefer-text-block`](./prefer-text-block.md),
   [`print-macro-split`](./print-macro-split.md), and
   [`format-macro-wrap`](./format-macro-wrap.md).
5. Emit a diagnostic suggesting the line-continuation rewrite:
   - Replace each interior `\n` in the template with
     `\n\<newline><indent>`.
   - For long stretches without `\n`, also break at the last
     whitespace within the budget, with a hard split at the
     boundary as fallback.
   - Move the template literal onto its own source line and
     close the attribute on a separate trailing line, matching
     the example above.

## Examples

```rust
// Bad: long source line in a #[display(...)] attribute
#[display("error: The error was caused by {_0}\nhint: Run {_1} to solve the problem")]
struct UserMessage(String, String);

// Good
#[display(
    "error: The error was caused by {_0}\n\
    hint: Run {_1} to solve the problem"
)]
struct UserMessage(String, String);
```

```rust
// Bad: long #[debug(...)] template
#[debug("Token {{ kind: {kind:?}, span: {span:?}, lexeme: {lexeme:?}, source_id: {source_id} }}")]
struct Token { /* ... */ }

// Good
#[debug(
    "Token {{ kind: {kind:?}, span: {span:?}, \
    lexeme: {lexeme:?}, source_id: {source_id} }}"
)]
struct Token { /* ... */ }
```

```rust
// Skipped: short source line
#[display("err: {code}")]
struct ErrCode(u32);

// Skipped: not a string literal template
#[display(fmt = renderer::DEFAULT_TEMPLATE)]
struct Custom;
```

## Configuration

```toml
[derive_more_template_wrap]
# Source-line width that triggers the rule. Default 100 matches
# rustfmt's column default. Width is unicode display width of the
# line containing the attribute, not its byte length.
max_line_width = 100

# Attribute paths whose first argument is a format template.
# Defaults cover the canonical derive_more attributes; extend
# this for project-specific derive_more wrappers that share the
# same template shape.
attribute_paths = ["display", "debug"]
```

The match against `attribute_paths` uses the trailing path
segment only, so `#[display(...)]` and `#[derive_more::display(...)]`
both qualify.

## Implementation notes

- `EarlyLintPass::check_attribute` reading the raw attribute
  tokens. The attribute's argument list is available
  pre-expansion as a `MetaItem` / `NestedMeta` tree.
- For each attribute whose path's last segment is in
  `attribute_paths`:
  - Walk the meta-item arguments. The first positional argument
    must be a string literal; record its span.
  - Compute the unicode display width of the *entire* attribute's
    source span (`#[...]` brackets included), not just the
    literal. Use the same `unicode-width` helper as
    [`prefer-text-block`](./prefer-text-block.md),
    [`print-macro-split`](./print-macro-split.md), and
    [`format-macro-wrap`](./format-macro-wrap.md).
  - Compare with `max_line_width`; emit if exceeded.
- **Parser style.** Implement the template scanner as parser-
  combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md).
  Reuse the placeholder/literal helpers from
  [`derive-more-inlined-args`](./derive-more-inlined-args.md),
  the escape scanner in `src/rules/prefer_raw_string.rs`,
  [`print-macro-split`](./print-macro-split.md), and
  [`format-macro-wrap`](./format-macro-wrap.md). The split logic
  is the same as `format-macro-wrap`'s: scan for the last
  whitespace within the budget, hard-split at the boundary as
  fallback.

### Difficulty

**Medium.** Same shape as
[`format-macro-wrap`](./format-macro-wrap.md): pure syntactic
reformat of the template literal, no argument re-slicing.
The only complication is splitting the attribute across multiple
source lines, which is sometimes more involved than splitting
a macro call (attributes are parsed by the derive macro and
some derive_more versions may be picky about whitespace inside
the meta-item parens).

Autofix is `MachineApplicable` for the literal-only rewrite (the
template content is byte-equivalent to the original); demoted to
`MaybeIncorrect` when the attribute also carries other
non-default arguments (`bound = "..."`, `forward`, etc.) whose
re-formatting would interact with the rewrite.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Default state

Active by default.

## Interaction with sibling rules

The four width-driven template rules together cover every place a
too-wide format template appears in source:

- [`prefer-text-block`](./prefer-text-block.md) — bare string
  literals not interpreted as templates.
- [`print-macro-split`](./print-macro-split.md) — splittable
  side-effect macros (`println!`, `writeln!`, `log::*!`).
- [`format-macro-wrap`](./format-macro-wrap.md) — value-producing
  or terminating macros (`format!`, `panic!`, `assert!`).
- `derive_more_template_wrap` (this rule) — derive_more
  attribute-form templates (`#[display(...)]`, `#[debug(...)]`).

Each rule's target set is disjoint from the others by design;
a given expression or attribute is the responsibility of exactly
one of the four. The shared
[`derive-more-inlined-args`](./derive-more-inlined-args.md) rule
operates at a different layer (placeholder syntax inside the
template), and runs orthogonally to all four.
