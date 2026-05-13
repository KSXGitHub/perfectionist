# `lint_downgrade_reason`

**Source:** project convention.

## Statement

A lint-level attribute that *relaxes* a lint — silences it
(`#[allow]`, `#[expect]`) or downgrades it from a stricter
inherited level (e.g., `#[warn]` placed under a crate-level
`#![deny]`) — must carry an explanatory `reason = "..."` field.
The strict forms (`#[deny]`, `#[forbid]`, or a `#[warn]` that
does not relax anything) are not flagged.

## Why restrict this?

This is a stylistic preference, not a correctness issue.

- Suppressions outlive the conditions that justify them. A bare
  `#[allow(clippy::too_many_arguments)]` told the original author
  to ignore a complaint; six months later, no one knows whether
  the rationale was "matches upstream signature", "intentional
  over-engineering", or "we'll fix it in the next refactor". The
  `reason` field records intent at the moment of suppression.
- Diagnostics render the `reason` in the lint message when the
  suppression is queried — e.g., the
  `unfulfilled_lint_expectations` note on a stale `#[expect]`
  shows the original rationale alongside the "no longer
  applies" message.
- The asymmetry — strict levels do not need a reason, relaxed
  levels do — matches the asymmetry of risk. Tightening a lint
  is routine project policy and its rationale tends to live in
  the project config alongside other policy. Relaxing it is a
  local exception, and the rationale belongs with the exception.

## Sub-lints

Two related triggers. Both register under the namespace
`perfectionist::lint_downgrade_reason::*`.

### `lint_downgrade_reason::silenced_without_reason`

For every `#[allow(<lints>)]` or `#[expect(<lints>)]` attribute
that does *not* contain a `reason = "..."` field, flag the
attribute.

`#[allow]` and `#[expect]` are the lowest two lint levels and
unconditionally silence output. They always count as a
relaxation regardless of the lint's default level — the author
has chosen "don't tell me about this", and the project's record
of suppressions needs to record why.

### `lint_downgrade_reason::downgraded_without_reason`

For every `#[warn(<lints>)]`, `#[allow(<lints>)]`, or
`#[expect(<lints>)]` attribute, compute the effective level of
each named lint at the *enclosing* scope (the project's
`dylint.toml` / `Cargo.toml` `[lints]` table, the crate-root
`#![deny(...)]`, the next outer item's `#[warn(...)]`, etc.). If
the new level is strictly lower than the enclosing level
(`deny → warn`, `deny → allow`, `deny → expect`, `warn → allow`,
`warn → expect`) and the attribute does not contain
`reason = "..."`, flag it.

This sub-lint subsumes `silenced_without_reason` when the lint's
inherited level is `warn` or `deny`. The two are kept separate so
that a project that finds the inherited-level analysis too noisy
or too expensive can enable `silenced_without_reason` alone (which
is purely local) and skip `downgraded_without_reason`.

## Examples

### `silenced_without_reason`

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

### `downgraded_without_reason`

```rust
#![deny(clippy::missing_errors_doc)]

// Bad — downgrades from the crate-level `deny` without a reason
#[warn(clippy::missing_errors_doc)]
pub fn parse(input: &str) -> Result<Manifest, ParseError> { /* ... */ }

// Good
#[warn(
    clippy::missing_errors_doc,
    reason = "stub during the parser-rewrite migration; tighten back to deny in #1234",
)]
pub fn parse(input: &str) -> Result<Manifest, ParseError> { /* ... */ }
```

```rust
// `#[deny]` and `#[forbid]` are never flagged by this rule — they
// tighten, not loosen.
#[deny(clippy::too_many_arguments)]
mod hot_path {}
```

## Autofix

Both sub-lints suggest inserting `, reason = ""` immediately
before the closing `)` of the argument list.
`Applicability::HasPlaceholders` — the empty string is a
placeholder the author fills in.

If the attribute uses bare zero-argument form
(`#[allow(my_lint)]` with no trailing comma), the suggestion
adds the comma. If the attribute is laid out across multiple
lines, the suggestion keeps the `reason` entry on its own line
matching the existing indentation.

## Configuration

```toml
[lint_downgrade_reason]
# Enable each sub-lint independently.
silenced_without_reason = "warn"   # or "allow", "deny", "forbid"
downgraded_without_reason = "warn"

# Lints excluded from the requirement. Useful for project-wide
# suppressions whose rationale lives in the project README rather
# than per-site.
exempt_lints = [
    # "clippy::module_name_repetitions",
]

# Minimum length of the `reason` value. A one-word reason
# ("legacy", "TODO") satisfies the literal requirement but conveys
# little; this knob enforces a useful floor. Set to 0 to disable.
min_reason_length = 3
```

## Implementation notes

- `silenced_without_reason`:
  `EarlyLintPass::check_attribute`. Match `attr.path` against
  `sym::allow` and `sym::expect`. Iterate
  `attr.meta_item_list()` and check whether any item has the
  shape `reason = "<str>"`. If absent, emit at the attribute's
  span.
- `downgraded_without_reason`: requires the effective ambient
  lint level at the attribute's site. Use the late pass: hook
  `LateLintPass::check_attribute` with `TyCtxt` available, and
  query `tcx.lint_level_at_node(lint, hir_id)` against the
  attribute's enclosing item's `HirId`. The returned level
  reflects the level *that would apply if this attribute did not
  exist*; if the attribute's level is strictly lower, the rule
  fires.
- The "strictly lower" comparison uses the standard lint-level
  ordering: `Forbid > Deny > Warn > Expect ≈ Allow`. `Expect`
  and `Allow` are treated as equal for this comparison — both
  fully silence output.
- Per-named-lint reasoning: an attribute that names multiple
  lints (`#[allow(a, b)]`) is treated as a separate
  relaxation per name. Configuration entries in `exempt_lints`
  remove names from the per-attribute set; if every named lint
  is exempt, the attribute is not flagged.
- The `reason`-field-presence check is shared with the
  `silenced_without_reason` sub-lint and lives in
  `src/common.rs::attr_has_reason`.

### Difficulty

**Sub-lint `silenced_without_reason`: easy.** A single attribute
walk that looks for `#[allow(...)]` / `#[expect(...)]` and checks
whether any nested meta item matches the `reason = "<str>"`
shape. The autofix span is the closing `)` of the attribute
list, plus the inserted bytes `, reason = ""`.

**Sub-lint `downgraded_without_reason`: hard.** The inherited-
level analysis is the cost driver:

- Lint-level resolution at an arbitrary HIR node is not stable
  across rustc nightlies. `tcx.lint_level_at_node` works today
  but has shifted signature across versions, so the rule has to
  pin to a compatible nightly or guard the call with a
  version-conditional shim.
- The ambient level must reflect *both* attribute-driven levels
  (`#![deny(...)]` on the crate, `#[warn(...)]` on a parent
  module) *and* configuration-driven levels (`dylint.toml`'s
  `[clippy_lints]` table, the workspace `Cargo.toml`'s `[lints]`
  table introduced in Rust 1.74, and the `RUSTFLAGS=-D <lint>`
  environment variable). Most of these are aggregated by
  `lint_level_at_node` before it returns, but the test matrix
  for the rule expands accordingly.
- Determining "the level that would apply if this attribute did
  not exist" requires temporarily removing the attribute from
  consideration. Either re-query the lint-level map against the
  attribute's parent `HirId`, or — equivalent and cheaper — walk
  the ancestry manually, skipping the attribute under inspection.
  The manual walk also handles `cfg_attr`-wrapped lint
  attributes uniformly, per the pattern in
  `src/enclosing_hir.rs`.

Ship `silenced_without_reason` first; `downgraded_without_reason`
in a follow-up once the lint-level query interface is pinned.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Severity

Warn.

## Interaction with sibling rules

- [`lint-reason-from-comment`](./lint-reason-from-comment.md) lifts
  an adjacent comment into the attribute's `reason` field. When
  the author has already written the rationale as a comment,
  that rule fires first and satisfies this one preemptively.
- [`prefer-expect-over-allow`](./prefer-expect-over-allow.md)
  rewrites `#[allow]` to `#[expect]`. The
  `silenced_without_reason` trigger applies equally to both
  forms, so the two rewrites compose: the canonical end state
  is `#[expect(<lint>, reason = "...")]`.
