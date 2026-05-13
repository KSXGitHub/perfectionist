# `prefer_expect_over_allow`

**Source:** project convention.

## Statement

Prefer `#[expect(...)]` over `#[allow(...)]` wherever the
expectation will be fulfilled. `#[expect]` triggers
`unfulfilled_lint_expectations` at the next compilation if the
lint stops firing — the suppression becomes self-cleaning.
`#[allow]` stays silent forever, including after the underlying
issue is resolved.

## Why restrict this?

This is a stylistic preference, not a correctness issue.

- A suppression often outlives the problem it suppressed.
  `#[allow]` has no signal when the underlying lint stops firing,
  so a project accumulates `#[allow]` attributes that no longer
  apply.
- `#[expect]` emits `unfulfilled_lint_expectations` the moment the
  named lint stops triggering at the site — exactly when the
  suppression becomes dead. Routine compilation tells the author
  to remove it.
- The change is local: every `#[expect]` is also a self-test that
  the lint *does* fire at the site, so a future refactor that
  inadvertently fixes the issue is observed rather than hidden.
- `#[allow]` remains appropriate when the lint cannot be relied on
  to fire deterministically — see "When `#[allow]` stays". This
  rule does not blanket-replace.

## What to lint

For every `#[allow(<lints>, ...)]` attribute — including the
inner-attribute form `#![allow(...)]` and the `cfg_attr`-wrapped
form `#[cfg_attr(<cfg>, allow(...))]` — if every lint named in
the attribute is one of:

- A built-in rustc lint (`unused_variables`, `dead_code`, …) not
  on the exempt list below.
- A `clippy::*` lint.
- A `rustdoc::*` lint.
- A `perfectionist::*` lint (or any tool namespace whose lints
  follow the standard fire-deterministically semantics).

…propose `#[expect(...)]` in its place. If the attribute mixes a
rewriteable lint with a non-rewriteable one (an exempt-list entry
or an unknown lint, both treated as non-rewriteable for split
purposes — e.g., `#[allow(dead_code, clippy::too_many_arguments)]`),
split it: keep the non-rewriteable names under `#[allow]`, move
the rewriteable ones to a new `#[expect]`.

## When `#[allow]` stays

Two cases the rule does not flag:

- The attribute names a lint that is known not to fire
  deterministically. The default exempt set is the
  `cfg`-conditional `unused_*` and reachability lints:
  `dead_code`, `unused_imports`, `unused_macros`,
  `unused_variables`, `unused_mut`, `unused_assignments`,
  `unused_must_use`, and `unreachable_code`. All of these can
  fire under one `cfg` arm and stay silent under another, so a
  mechanical `expect` rewrite would break the build in the
  silent arm. Projects extend or trim this set via
  `exempt_lints`.
- The attribute is `#![allow(...)]` at the crate root or on a
  whole module. `cfg`-conditional bodies inside the module may
  individually fire or not fire the lint, and `#[expect]` at this
  scope is fulfilled only if *some* item below it fires the lint
  — a fragile invariant. Configurable; default `false`.

## Examples

The examples below already carry a `reason` field so that the
only difference between Bad and Good is the `allow` → `expect`
swap. The `reason`-presence requirement is enforced by the
sibling [`lint-silence-reason`](./lint-silence-reason.md), not
by this rule; an `#[allow]` without `reason` would be flagged by
both rules independently.

```rust
// Bad
#[allow(clippy::too_many_arguments, reason = "matches pnpm's signature")]
fn build_fetcher(/* ... */) {}

// Good
#[expect(clippy::too_many_arguments, reason = "matches pnpm's signature")]
fn build_fetcher(/* ... */) {}
```

```rust
// Not flagged — `dead_code` is on the default exempt list because
// a `cfg(test)`-only use may keep the item alive in some builds
// but not others.
#[allow(dead_code, reason = "used only in integration tests")]
struct InternalState;
```

```rust
// Split — the rewriteable half moves; the exempt half stays.
#[allow(
    dead_code,
    clippy::too_many_arguments,
    reason = "scaffolding for the upcoming feature",
)]
fn scaffold(/* ... */) {}

// →
#[allow(dead_code, reason = "scaffolding for the upcoming feature")]
#[expect(
    clippy::too_many_arguments,
    reason = "scaffolding for the upcoming feature",
)]
fn scaffold(/* ... */) {}
```

## Autofix

Replace the attribute path identifier `allow` with `expect`. The
rewrite span is the five bytes `allow`.

For a `cfg_attr`-wrapped lint attribute, the span targets the
inner `allow` identifier inside the `cfg_attr` argument list,
not the outer `cfg_attr` path. The cfg-arm scoping is preserved
unchanged; only the inner level identifier moves.

For the split case (mixed exempt and rewriteable lints), the
autofix is a multi-attribute rewrite: replace the original
attribute with two attributes, copying the `reason` field to each.

`Applicability::MaybeIncorrect`. Even though the rewrite is
mechanical, the cases where `#[expect]` is unsuitable
(non-deterministic firing, `cfg`-gated bodies) cannot be detected
from the call site alone. The autofix is shown as a help
suggestion rather than applied automatically by `cargo fix`.

## Configuration

```toml
[prefer_expect_over_allow]
# Lints that are exempt — `#[allow]` for these stays. Names are
# matched against the fully-namespaced lint name shown in
# diagnostics (e.g. `clippy::too_many_arguments`).
exempt_lints = [
    "dead_code",
    "unused_imports",
    "unused_macros",
    "unused_variables",
    "unused_mut",
    "unused_assignments",
    "unused_must_use",
    "unreachable_code",
]

# When true, also rewrite crate-level `#![allow(...)]` and
# module-level `#[allow(...)]` attributes. Default `false`
# because `cfg`-conditional bodies inside the scope are common.
apply_to_outer_scopes = false

# When false, only `clippy::*`, `rustdoc::*`, and built-in lints
# are rewritten; tool namespaces (`perfectionist::*` and similar)
# are left alone.
apply_to_tool_namespaces = true
```

## Implementation notes

- `EarlyLintPass::check_attribute`. Match `AttrKind::Normal` with
  `attr.path == sym::allow` and a non-empty
  `attr.meta_item_list()`. For `cfg_attr`-wrapped lint
  attributes, walk into the `cfg_attr` argument list and apply
  the same match to each inner attribute — `src/enclosing_hir.rs`
  carries the established walker shape; reuse it here.
- For each nested meta item, classify the lint name:
  - **Built-in:** the name resolves through `LintStore::find_lints`
    to a lint registered with no tool prefix.
  - **`clippy::*` / `rustdoc::*` / `perfectionist::*`:**
    tool-prefixed via `MetaItem::path.segments`.
  - **Unknown:** bail; the lint may be a procedural plugin that
    fires conditionally.
- The default rewrite span covers only the `allow` identifier
  inside `attr.path.segments[0].ident.span`. The split-attribute
  rewrite spans the whole attribute and renders two replacement
  attributes verbatim.

### Difficulty

**Medium.** Detection is straightforward: read the attribute path,
list the inner lint names, classify each. The simple autofix is
a one-identifier substitution (`allow` → `expect`).

What pushes this past "easy" is the eligibility decision. Some
lints fire only in some compilations — `dead_code`, `unused_*`,
`clippy::ptr_arg` under certain build configurations. Mechanically
rewriting them to `#[expect]` turns a quiet suppression into a
hard error in any configuration where the lint does not fire. The
curated exempt list mitigates this for the well-known cases; the
rule's autofix defaults to `MaybeIncorrect`. A project that
enforces this rule in CI should run the cleanup behind a separate
`cargo dylint --fix` invocation rather than batching it into a
routine formatter pass.

The split-attribute path (mixed exempt + rewriteable inputs) is
the second source of complexity: the original attribute's
`reason` field has to be copied to both replacement attributes,
and any trailing comma / multi-line formatting has to be
preserved. The rewrite is doable as a textual splice on the
original span but the bookkeeping is worth its own helper.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Severity

Warn.

## Interaction with sibling rules

- [`lint-reason-from-comment`](./lint-reason-from-comment.md) lifts
  an adjacent comment into the attribute's `reason` field before
  this rule rewrites `allow` → `expect`. The two rewrites
  compose in either order.
- [`lint-silence-reason`](./lint-silence-reason.md) requires a
  `reason` field on every `#[allow]` and `#[expect]`. After this
  rule rewrites `allow` → `expect`, the `reason` requirement
  still applies; the two rules together produce the canonical
  form `#[expect(<lint>, reason = "...")]`.
- [`lint-downgrade-reason`](./lint-downgrade-reason.md) is
  orthogonal: it cares about the level relative to the
  inherited level, and `allow` and `expect` rank equally in
  that comparison, so this rule's rewrite does not change
  whether it fires.
