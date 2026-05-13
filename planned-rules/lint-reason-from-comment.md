# `lint_reason_from_comment`

**Source:** project convention.

## Statement

When a lint-level attribute (`#[allow]`, `#[expect]`, `#[warn]`,
`#[deny]`, `#[forbid]`) carries an adjacent comment that documents
*why* the level was chosen, the comment belongs inside the
attribute itself as a `reason = "..."` field. Lift the prose into
the attribute and delete the comment.

## Why restrict this?

This is a stylistic preference, not a correctness issue.

- `reason = "..."` is part of the attribute and travels with it
  through every refactor: a `cargo expand`, a copy-paste, an
  attribute that moves to a different item. A free-floating
  comment can be separated from its attribute by an unrelated
  edit without warning.
- Compiler diagnostics render the `reason` field in the lint's
  message (`note: rationale: ...`), so the explanation reaches
  the reader at the moment of confusion. An adjacent comment is
  visible only in the source.
- One canonical location for the rationale removes the question
  "is this comment for the attribute, or for the next item?"
- The `reason` field was stabilised in Rust 1.81 for this exact
  purpose. Comments-as-reasons predate the field; the catalogue
  is prescribing the modern form.

## What to lint

For every lint-level attribute (`allow`, `expect`, `warn`, `deny`,
`forbid`) without an existing `reason = "..."` field, look for an
adjacent comment:

- **Trailing comment** on the same source line as the attribute's
  closing `]`. The canonical placement and the highest-confidence
  case.
- **Leading comment** on the previous source line, with no blank
  line between the comment and the attribute and no other
  attribute in between. Lower confidence — the comment may also
  be documentation for the next item.

If the attribute already contains `reason = "..."`, skip — there
is nothing to lift.

The rule is intentionally narrow about which comment forms count.
Doc comments (`///`, `//!`) are *not* in scope; those document
the attached item, not the attribute. Block comments (`/* ... */`)
embedded mid-attribute are skipped — the placement is too unusual
to mechanically rewrite.

## Examples

```rust
// Bad — trailing comment as reason
#[allow(clippy::too_many_arguments)] // arg count is set by upstream pnpm's fetcher signature
fn build_fetcher(/* ... */) {}

// Good
#[allow(
    clippy::too_many_arguments,
    reason = "arg count is set by upstream pnpm's fetcher signature",
)]
fn build_fetcher(/* ... */) {}
```

```rust
// Bad — leading comment as reason
// pnpm's wire format is JSON-stringly typed
#[allow(clippy::struct_excessive_bools)]
pub struct Manifest { /* ... */ }

// Good
#[allow(
    clippy::struct_excessive_bools,
    reason = "pnpm's wire format is JSON-stringly typed",
)]
pub struct Manifest { /* ... */ }
```

```rust
// Not flagged — the doc comment documents the function, not the
// attribute.
/// Builds a fetcher for the configured registry.
#[allow(clippy::too_many_arguments, reason = "matches pnpm signature")]
fn build_fetcher(/* ... */) {}
```

## Autofix

Lift the comment text into `reason = "..."`, then delete the
comment.

Recognised comment shapes: a single `//` line-comment
immediately adjacent to the attribute, in either the trailing or
leading position defined in "What to lint". Doc comments
(`///`, `//!`) are filtered out at the trigger stage (see "What
to lint"). Multi-line trailing comments (a `// ...` followed by
another `// ...` on the next line) are *not* in scope — only the
single comment immediately adjacent to the attribute is lifted;
the second line is left alone.

Text normalisation:

- Strip the leading `//` marker.
- Trim surrounding ASCII whitespace.
- Strip a single leading run of one or more `-`, `=`, or `*`
  decoration characters followed by whitespace
  (`//-- foo` / `//== foo` / `//* foo` → `foo`). Other decoration
  styles pass through unchanged; the author can clean them up
  after the autofix.
- Escape `\\`, `"`, and control characters per Rust string-literal
  rules. The escape helper is described under "Implementation
  notes".

Splice into the attribute:

- Insert `, reason = "<normalised, escaped text>"` immediately
  before the closing `)` of the attribute's argument list (for a
  `cfg_attr`-wrapped attribute, the *inner* lint attribute's
  closing `)`, not the outer `cfg_attr`'s).
- Delete the original comment span. If the comment was on its
  own line and removing it leaves the line blank, delete the
  whole line.

`Applicability::MachineApplicable` for trailing comments — the
attachment is unambiguous. `Applicability::MaybeIncorrect` for
leading comments — the comment might really belong to the next
item; the author confirms.

## Configuration

```toml
[lint_reason_from_comment]
# Comment placements considered candidates. Subset of these two.
sites = ["trailing", "leading"]
```

## Implementation notes

- `EarlyLintPass::check_attribute`. Use
  `src/common.rs::attr_has_reason` to check whether the
  attribute already has a `reason` field (shared with
  [`lint-silence-reason`](./lint-silence-reason.md) and
  [`lint-downgrade-reason`](./lint-downgrade-reason.md)); if
  present, skip.
- For trailing-comment detection: read the attribute's span, walk
  the source map forward over horizontal whitespace, and check
  whether the next token on the same line is a line comment.
- For leading-comment detection: walk backward from the
  attribute's start over horizontal whitespace, then look for the
  end of a line comment terminated by a newline with no blank line
  separating the two.
- Reuse the regular-comment retokenizer from the implemented
  `perfectionist::unicode_ellipsis_in_comments` lint (see
  `src/rules/unicode_ellipsis_in_comments.rs`) for locating
  comments by source range.
- The Rust-string-literal escape helper used to render the
  lifted comment as a `"..."` string is single-use today — the
  sibling reason-related rules insert an empty string and need
  no escape. Per the catalogue's convention (single-rule helpers
  live in the rule's own file, not `src/common.rs`), keep it
  private to this rule's module until a second consumer arrives.

### Difficulty

**Easy.** Two single-pass source-map searches around each lint
attribute (one for the trailing comment, one for the leading
comment) plus a `MetaItem` lookup to check for an existing
`reason` field. The Rust-string-literal escape used by the
autofix is the only piece of non-trivial logic, and the
`std::ascii::escape_default`-equivalent shape is around twenty
lines.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Severity

Warn.

## Interaction with sibling rules

- [`prefer-expect-over-allow`](./prefer-expect-over-allow.md) acts
  on the same attributes but does not touch the `reason` field.
  The two rewrites compose in either order.
- [`lint-silence-reason`](./lint-silence-reason.md) requires
  that every `#[allow]` / `#[expect]` carry a `reason` field;
  [`lint-downgrade-reason`](./lint-downgrade-reason.md) extends
  the same requirement to `#[warn]` / `#[allow]` / `#[expect]`
  that lower an inherited level. When the rationale is
  currently written as a comment, this rule fires first and
  lifts the comment; the sibling rules then see the `reason`
  field and stay silent.
