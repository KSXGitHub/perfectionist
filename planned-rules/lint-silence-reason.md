# `lint_silence_reason`

**Source:** project convention.

## Statement

Every `#[allow(<lints>)]` and `#[expect(<lints>)]` attribute must
carry an explanatory `reason = "..."` field. `#[allow]` and
`#[expect]` are the two levels that fully silence a lint's
output; the project's record of suppressions needs to know why
each one exists.

The check is purely local — the attribute itself — and does not
depend on any inherited or ambient lint level. The
ancestry-aware case (a `#[warn]` lowering an inherited `#[deny]`,
etc.) is covered by the sibling
[`lint-downgrade-reason`](./lint-downgrade-reason.md).

## Why restrict this?

This is a stylistic preference, not a correctness issue.

- Suppressions outlive the conditions that justify them. A bare
  `#[allow(clippy::too_many_arguments)]` told the original
  author to ignore a complaint; six months later, no one knows
  whether the rationale was "matches upstream signature",
  "intentional over-engineering", or "we'll fix it in the next
  refactor". The `reason` field records intent at the moment of
  suppression.
- Diagnostics render the `reason` in the lint message when the
  suppression is queried — e.g., the
  `unfulfilled_lint_expectations` note on a stale `#[expect]`
  shows the original rationale alongside the "no longer applies"
  message.
- The rule is the cheapest in this cluster to enforce
  (`EarlyLintPass`, no resolution, no ancestry walk) and the
  one with the highest hit rate in practice: most suppressions
  in a typical codebase are of clippy lints whose default level
  is `warn`, all of which are silenced by `#[allow]` /
  `#[expect]`.

## What to lint

For every `#[allow(<lints>, ...)]` or `#[expect(<lints>, ...)]`
attribute, first filter against `exempt_lints`: if every lint
named in the attribute is on the configured exempt list, the
attribute is not flagged. Otherwise, check whether any nested
meta item has the shape `reason = "<str>"`:

- **Absent.** Emit a "missing reason" diagnostic at the
  attribute's span.
- **Present but shorter than `min_reason_length` (default 3).**
  Emit a "reason too short" diagnostic at the `reason` literal's
  span. A one-character `reason = "x"` satisfies the literal
  presence requirement but conveys no rationale; the length
  floor exists to catch this. Set `min_reason_length = 0` to
  disable the length branch entirely (presence still enforced).
- **Present and long enough.** Accept.

`#[warn]`, `#[deny]`, and `#[forbid]` are not in scope for this
rule. A `#[warn]` that lowers an inherited `#[deny]` *is* a
relaxation and needs a reason, but detecting it requires the
ancestry walk that
[`lint-downgrade-reason`](./lint-downgrade-reason.md) owns.

## Examples

```rust
// Bad
#[allow(clippy::too_many_arguments)]
fn build_fetcher(/* ... */) {}

// Good
#[allow(clippy::too_many_arguments, reason = "matches pnpm's signature")]
fn build_fetcher(/* ... */) {}
```

```rust
// Bad — `expect` without reason
#[expect(clippy::too_many_arguments)]
fn build_fetcher(/* ... */) {}

// Good
#[expect(clippy::too_many_arguments, reason = "matches pnpm's signature")]
fn build_fetcher(/* ... */) {}
```

```rust
// Not flagged by this rule — `#[warn]` is out of scope here.
// `lint-downgrade-reason` decides whether the `warn` is a
// relaxation relative to ambient policy.
#[warn(clippy::missing_errors_doc)]
pub fn parse(/* ... */) {}
```

## Autofix

For the missing-reason case: insert `, reason = ""` immediately
before the closing `)` of the attribute's argument list.
`Applicability::HasPlaceholders` — the empty string is a
placeholder the author fills in.

For a `cfg_attr`-wrapped lint attribute
(`#[cfg_attr(<cfg>, allow(...))]`), the closing `)` targeted is
the *inner* `allow(...)` / `expect(...)` argument list, not the
outer `cfg_attr` one. The inner-attribute form `#![allow(...)]`
uses the same insertion point as the outer form.

If the attribute is laid out across multiple lines, the
suggestion keeps the `reason` entry on its own line matching the
existing indentation.

The too-short case has no autofix: the lint cannot synthesise a
longer rationale on the author's behalf. The diagnostic points
at the `reason` literal's span; the author rewrites it.

## Configuration

```toml
[lint_silence_reason]
# Lints excluded from the requirement. Useful for project-wide
# suppressions whose rationale lives in the project README rather
# than per-site.
exempt_lints = [
    # "clippy::module_name_repetitions",
]

# Minimum length of the `reason` value. A one-word reason
# ("legacy", "TODO") satisfies the literal requirement but
# conveys little; this knob enforces a useful floor. Set to 0 to
# disable.
min_reason_length = 3
```

## Implementation notes

- `EarlyLintPass::check_attribute`. Match `attr.path` against
  `sym::allow` and `sym::expect`. For `cfg_attr`-wrapped lint
  attributes, walk into the `cfg_attr` argument list and apply
  the same match to each inner attribute — `src/enclosing_hir.rs`
  carries the established walker shape; reuse it here.
- Iterate `attr.meta_item_list()` (or the equivalent inner
  meta-item list for `cfg_attr`-wrapped attributes) and check
  whether any item has the shape `reason = "<str>"`. If absent,
  emit at the attribute's span.
- Per-named-lint handling: an attribute that names multiple
  lints (`#[allow(a, b)]`) is treated as a single relaxation
  for diagnostic purposes — one missing `reason` triggers one
  message regardless of how many lints the attribute lists.
  Configuration entries in `exempt_lints` filter the
  per-attribute set; if every named lint is exempt, the
  attribute is not flagged.
- The `reason`-presence check is shared with
  [`lint-reason-from-comment`](./lint-reason-from-comment.md)
  and
  [`lint-downgrade-reason`](./lint-downgrade-reason.md). Factor
  it into `src/common.rs::attr_has_reason` the first time any
  of the three rules lands.

### Difficulty

**Easy.** A single attribute walk that reads the meta-item list
for `reason = "<str>"`. The autofix span is the closing `)` of
the attribute list, plus the inserted bytes `, reason = ""`.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Severity

Warn.

## Interaction with sibling rules

- [`lint-reason-from-comment`](./lint-reason-from-comment.md)
  lifts an adjacent comment into the attribute's `reason`
  field. When the author has already written the rationale as a
  comment, that rule fires first and satisfies this one
  preemptively.
- [`prefer-expect-over-allow`](./prefer-expect-over-allow.md)
  rewrites `#[allow]` to `#[expect]`. The two rewrites apply to
  overlapping attribute sets; the canonical end state is
  `#[expect(<lint>, reason = "...")]`.
- [`lint-downgrade-reason`](./lint-downgrade-reason.md) covers
  the ancestry-aware case (a `#[warn]` lowering an inherited
  `#[deny]`, etc.). The two rules together cover every
  relaxation: local silencing here, relative downgrade there.
  When both are enabled, an `#[allow]` / `#[expect]` whose
  inherited level is `warn` or `deny` is flagged by both — the
  rules deduplicate via a shared attribute-already-flagged
  guard.
