# `lint_downgrade_reason`

**Source:** project convention.

## Statement

A lint-level attribute that lowers a lint's level *relative to
the inherited level* — `#[warn]` placed under a crate-level
`#![deny]`, `#[allow]` overriding an outer `#[warn]`, and so on —
must carry an explanatory `reason = "..."` field. Attributes that
tighten, or that match the inherited level, are not flagged.

The local case — every `#[allow]` / `#[expect]` regardless of
inherited level — is covered by the sibling
[`lint-silence-reason`](./lint-silence-reason.md). This rule
covers the ancestry-aware case: any explicit step down relative
to the surrounding scope.

## Why restrict this?

This is a stylistic preference, not a correctness issue.

- A relative downgrade is a *local exception* to a *project
  policy*. The policy tends to live in `dylint.toml` /
  `Cargo.toml`'s `[lints]` table / a crate-root `#![deny(...)]`;
  the exception lives at the site that needs it. The rationale
  for the exception is itself local information and belongs
  with the exception.
- The asymmetry — strict levels do not need a reason, relaxed
  levels do — matches the asymmetry of risk. Tightening is
  routine policy and its rationale lives in the config file.
  Relaxing the project policy at a specific site is the kind of
  change a reviewer wants context for.
- A grep for `#[warn(<lint>)]` is the easiest way to find
  partial migrations away from a project-wide `#[deny(<lint>)]`
  policy. Embedding the reason makes those greps
  self-documenting ("stub during the parser-rewrite migration;
  tighten back to deny in #1234").

## What to lint

For every `#[warn(<lints>)]`, `#[allow(<lints>)]`, or
`#[expect(<lints>)]` attribute, compute the effective level of
each named lint at the *enclosing* scope (the project's
`dylint.toml` / `Cargo.toml` `[lints]` table, the crate-root
`#![deny(...)]`, the next outer item's `#[warn(...)]`, etc.). If
the new level is strictly lower than the enclosing level
(`deny → warn`, `deny → allow`, `deny → expect`, `warn → allow`,
`warn → expect`) and the attribute does not contain
`reason = "..."`, flag it.

`#[deny]` and `#[forbid]` are never flagged — they tighten, not
loosen. `#[warn]` at a site whose inherited level is already
`warn` is not flagged either: the attribute is a no-op relative
to ambient policy and does not relax anything.

The lint-level ordering used for "strictly lower" is
`Forbid > Deny > Warn > Expect ≈ Allow`. `Expect` and `Allow`
are treated equally — both fully silence output.

## Relationship to `lint-silence-reason`

The two rules overlap when the inherited level is `warn` or
`deny`:

- [`lint-silence-reason`](./lint-silence-reason.md) fires on
  every `#[allow]` / `#[expect]` regardless of inherited level.
- `lint_downgrade_reason` fires on every level lower than
  ambient, which includes `#[allow]` / `#[expect]` whose
  inherited level is `warn` or `deny` — and additionally
  catches `#[warn]` over `#[deny]`, which the sibling rule
  cannot see.

A project that enables both rules gets one diagnostic per
attribute either way (the rules deduplicate via a shared
attribute-already-flagged guard). A project that enables only
`lint_silence_reason` skips the ancestry walk entirely and still
catches the high-value cases — most silencing in practice is of
clippy lints whose default level is `warn`. A project that
enables only `lint_downgrade_reason` accepts `#[allow]` on
default-`allow` lints (rare) but catches every relative
downgrade including `#[warn]` over `#[deny]`.

## Examples

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
// `#[deny]` and `#[forbid]` are never flagged — they tighten,
// not loosen.
#[deny(clippy::too_many_arguments)]
mod hot_path {}
```

```rust
// Not flagged — `#[warn]` does not lower the lint below its
// inherited `warn`, so the attribute is a no-op relative to
// ambient policy.
#[warn(clippy::too_many_arguments)]
fn build(/* ... */) {}
```

## Autofix

Insert `, reason = ""` immediately before the closing `)` of the
argument list. `Applicability::HasPlaceholders` — the empty
string is a placeholder the author fills in. Layout is preserved
the same way as for
[`lint-silence-reason`](./lint-silence-reason.md).

## Configuration

```toml
[lint_downgrade_reason]
# Lints excluded from the requirement.
exempt_lints = [
    # "clippy::module_name_repetitions",
]

# Minimum length of the `reason` value. A one-word reason
# ("legacy", "TODO") satisfies the literal requirement but
# conveys little; this knob enforces a useful floor. Set to 0
# to disable.
min_reason_length = 3
```

## Implementation notes

- `LateLintPass::check_attribute` with `TyCtxt` available. Match
  `attr.path` against `sym::warn`, `sym::allow`, `sym::expect`.
- For each named lint, resolve its inherited level using
  `tcx.lint_level_at_node(lint, parent_hir_id)` where
  `parent_hir_id` is the HIR node *above* the attribute's owner
  — equivalent to "what level would apply if this attribute did
  not exist". The manual walk also handles `cfg_attr`-wrapped
  lint attributes uniformly, per the pattern in
  `src/enclosing_hir.rs`.
- Compare the attribute's level against the inherited level
  using the `Forbid > Deny > Warn > Expect ≈ Allow` ordering.
  Emit only when strictly lower.
- The `reason`-presence check is shared with
  [`lint-silence-reason`](./lint-silence-reason.md) — both
  consume `src/common.rs::attr_has_reason`.

### Difficulty

**Hard.** The inherited-level analysis is the cost driver:

- Lint-level resolution at an arbitrary HIR node is not stable
  across rustc nightlies. `tcx.lint_level_at_node` works today
  but has shifted signature across versions, so the rule has to
  pin to a compatible nightly or guard the call with a
  version-conditional shim.
- The ambient level must reflect *both* attribute-driven levels
  (`#![deny(...)]` on the crate, `#[warn(...)]` on a parent
  module) *and* configuration-driven levels (`dylint.toml`'s
  `[clippy_lints]` table, the workspace `Cargo.toml`'s
  `[lints]` table introduced in Rust 1.74, and the
  `RUSTFLAGS=-D <lint>` environment variable). Most of these
  are aggregated by `lint_level_at_node` before it returns, but
  the test matrix for the rule expands accordingly.
- Determining "the level that would apply if this attribute did
  not exist" requires temporarily removing the attribute from
  consideration. Either re-query the lint-level map against the
  attribute's parent `HirId`, or — equivalent and cheaper —
  walk the ancestry manually, skipping the attribute under
  inspection.

Ship [`lint-silence-reason`](./lint-silence-reason.md) first
(easy, `EarlyLintPass`, no ancestry); this rule in a follow-up
once the lint-level query interface is pinned for the target
nightly.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Severity

Warn.

## Interaction with sibling rules

- [`lint-silence-reason`](./lint-silence-reason.md) covers the
  local case (every `#[allow]` / `#[expect]` regardless of
  inherited level). See "Relationship to `lint-silence-reason`"
  above.
- [`lint-reason-from-comment`](./lint-reason-from-comment.md)
  lifts an adjacent comment into the attribute's `reason` field
  and so satisfies this rule preemptively when the rationale is
  already present in source.
- [`prefer-expect-over-allow`](./prefer-expect-over-allow.md)
  rewrites `#[allow]` to `#[expect]`. The level under analysis
  here is the relaxation level, so the rewrite does not affect
  whether this rule fires — `allow` and `expect` rank equally
  in the ordering above.
