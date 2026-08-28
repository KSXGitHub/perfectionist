# `collect_then_join`

**Source:** project convention, motivated by a real review of
[`HoangVanKhai/my-translated-lyrics#86`](https://github.com/HoangVanKhai/my-translated-lyrics/pull/86#discussion_r3447990805),
where an AI-generated helper built an intermediate `Vec<String>` only
to string-join it:

```rust
fn join_display(items: &[Item]) -> String
where
    Item: Display,
{
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
```

The workspace already depends on `itertools`, whose `Itertools::join`
does the same thing without the temporary `Vec`:

```rust
use itertools::Itertools;

fn join_display(items: &[Item]) -> String
where
    Item: Display,
{
    items.iter().join(", ")
}
```

## Statement

A `.collect::<Vec<_>>().join(sep)` chain allocates a `Vec` whose only
purpose is to be the receiver of the immediately-following `.join(sep)`.
`itertools::Itertools::join` consumes the iterator directly — it writes
each item into the result `String` through `Display` as it goes — so the
intermediate `Vec` disappears entirely.

The lint fires on the chain `<iter>.collect::<Vec<_>>().join(<sep>)`
when the `join` is the **string** specialization — separator typed
`&str`, result typed `String` — and suggests rewriting it to
`<iter>.join(<sep>)` against `itertools::Itertools`, adding
`use itertools::Itertools;` if it is not already in scope.

The string specialization is the load-bearing restriction: `<[T]>::join`
is overloaded. With a `&str` separator on a `[String]` / `[&str]` slice
it concatenates into a `String` (the case this rule targets), but with a
slice separator on a `[Vec<T>]` / `[&[T]]` slice it concatenates into a
`Vec<T>`. `itertools` has **no** equivalent for the slice-concatenating
form, so the rule must recognise and leave it alone — see
"[What to lint](#what-to-lint)".

## Why restrict this?

This is a stylistic preference, not a correctness issue. The two forms
produce an identical `String`; nothing is broken by collecting first.
The project prefers the `itertools` form because:

- **It drops a heap allocation.** `collect::<Vec<_>>()` allocates a
  backing buffer for every joined element and then throws it away the
  moment `join` has read it. `Itertools::join` writes straight into the
  one `String` it returns, so the only allocation is the result the
  caller wanted. (This is the "inefficient" framing of the source issue,
  but the result is identical either way, which is why this is filed
  under "restrict", not "bad".)
- **It removes a redundant stringification.** When the chain is
  `.map(ToString::to_string).collect::<Vec<_>>().join(sep)`, the `.map`
  exists only because `<[String]>::join` needs owned `String`s in the
  slice. `Itertools::join` takes any `Display` item, so the `.map`
  *and* the `.collect` both vanish: `items.iter().join(", ")`. The
  shorter chain says what it does — "join these by `, `" — without the
  buffer-and-stringify scaffolding.
- **It reads as one operation.** "Join an iterator with a separator" is
  a single intent; spelling it as collect-then-join splits it across two
  method calls and an explicit collection type that the reader has to
  recognise as throwaway.

## Workspace gate

The suggested fix names `itertools`, so by default the rule is silent in
workspaces that do not already depend on it — emitting the diagnostic
would propose a dependency the user cannot apply without a separate
`cargo add`. Under the default `require_itertools_dependency = true`,
the gate opens (the rule proceeds to its per-expression trigger) when
**any** of the following holds:

1. The workspace's root `Cargo.toml` declares `itertools` in
   `[workspace.dependencies]` (the only workspace-level dependency table
   cargo supports; member crates opt in with
   `itertools = { workspace = true }`).
2. The local crate's own `Cargo.toml` declares `itertools` in
   `[dependencies]` or `[dev-dependencies]`.
3. *Any* other crate in the same workspace declares `itertools` in
   `[dependencies]` or `[dev-dependencies]`.

The gate is evaluated once per `dylint` run, before per-file analysis,
and cached as a single boolean on the lint pass. When the gate is closed
the rule emits nothing; silence is the correct behaviour for a project
that has legitimately opted out by not depending on `itertools`, and no
"rule was skipped" warning is produced. (This is a per-expression gate,
separate from the catalogue-wide default-state mechanism in
[`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#rule-activation-model):
the rule is `Active by default` in the catalogue-wide sense; the gate
then controls whether each diagnostic fires.)

A project that re-exports `itertools` under a private wrapper, or that
wants the lint to fire ahead of adding the dependency, can set
`require_itertools_dependency = false` to force the gate open.

## Configuration

```toml
# dylint.toml
#
# Active by default. The rule has a single direction (prefer
# `Itertools::join` over an intermediate `collect`), so there is no
# `style` knob.
["perfectionist::collect_then_join"]

# Gate the rule on the workspace already depending on `itertools`.
# Defaults to `true`: a workspace with no `itertools` dependency anywhere
# stays silent (see "Workspace gate"). Set to `false` to fire regardless
# — e.g. when `itertools` is reached through a private re-export.
require_itertools_dependency = true
```

## What to lint

`LateLintPass`. Type resolution is required (to confirm the `join`
specialization and the `collect` target), so this is a late pass, not a
token scan.

`check_expr` on `ExprKind::MethodCall` whose method name is `join`. Fire
only when **all** of the following hold:

1. **The `join` is the string specialization.** The call's separator
   argument resolves to `&str` (a shared reference to `str`) **and** the
   call's result type resolves to `String`. This is what distinguishes
   `<[String]>::join(&self, &str) -> String` from the
   slice-concatenating `<[Vec<T>]>::join(&self, &[T]) -> Vec<T>`, which
   has no `itertools` equivalent and must not be flagged. Confirm the
   resolved `DefId` is the slice-`join` (`core`'s `[T]::join` via the
   `Join` trait), not `itertools`' own `join` — re-flagging
   already-fixed code would loop.
2. **The receiver is a `collect`.** `receiver.kind` is an
   `ExprKind::MethodCall` whose method is `collect`, with `DefId`
   resolving to `Iterator::collect`.
3. **The `collect` produces a `Vec`.** The collect expression's type
   resolves to `Vec<E>` (the only contiguous owned collection the
   default `.collect()` builds that also exposes `<[E]>::join`; a boxed
   slice or fixed array reaches `join` through the same `Deref`, but
   `.collect()` does not build those, so `Vec` is the only shape that
   appears here). Because step 1 already pinned the result to `String`,
   `E` is `String` or `&str` and so is `Display` — exactly what
   `Itertools::join` requires.

The diagnostic spans the `collect().join(...)` tail (from the `.collect`
method call through the close paren of `.join`) and suggests the
`itertools` rewrite below.

Guard against proc-macro-synthesised nodes per the
[suppression convention](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations):
a macro that
assembles a `collect().join()` chain from user-source fragments can
carry a user-source span that slips past `report_in_external_macro:
false`. Add `crate::common::hir_in_external_macro` and a
`ui/collect_then_join_proc_macro.rs` regression fixture (built around a
trigger the rule actually fires on, and mutation-checked by confirming
it fails with the guard removed).

## Autofix

The rewrite keeps the receiver iterator expression verbatim and ensures
`use itertools::Itertools;` is in scope (insert it if absent; leave it if
already imported, e.g. via a `prelude`). It takes one of these shapes:

- **Plain collect-then-join.** `iter.collect::<Vec<_>>().join(sep)` →
  `iter.join(sep)`. Drop the `.collect::<…>()` call and its turbofish;
  splice the original separator argument into the new `.join`.
  `MachineApplicable`.
- **Redundant-stringify collect-then-join.** When the receiver of the
  `collect` is itself `.map(ToString::to_string)` /
  `.map(|x| x.to_string())` and the *pre-`map`* item type is already
  `Display`, the `.map` is also redundant under `Itertools::join`:
  `iter.map(ToString::to_string).collect::<Vec<_>>().join(sep)` →
  `iter.join(sep)`. This branch is opt-in inside the autofix only when
  the pre-`map` item's `Display` impl is unambiguous; otherwise fall
  back to the plain rewrite (drop only the `collect`, keep the `map`,
  which is always valid since `String: Display`). Downgrade to
  `MaybeIncorrect` whenever the `.map` is stripped, since the reader
  should confirm the pre-`map` `Display` rendering matches.

The import-insertion edit follows the same machinery other
import-introducing rules in this catalogue use; if `itertools::Itertools`
is unresolvable in the file's scope (the workspace gate is open via a
re-export only), emit the diagnostic help-only without the
`use`-insertion suggestion.

## Difficulty

**Medium.** The trigger is local to one two-method chain, so there is no
whole-crate use-analysis. The work is in the type checks: distinguishing
the string `join` from the slice-concatenating `join` by separator and
result type (step 1) is what keeps the rule from false-positiving on
`Vec<Vec<_>>::join`, and the optional `.map(ToString::to_string)`-removal
branch needs a `Display` check on the pre-`map` item type. The workspace
gate reuses the manifest-probing pattern already established by
`manual_json_string`.

## Default state

Active by default, subject to the workspace gate above. The preferred
form has a single direction and the eligible shape is unambiguous, so
there is no neutral baseline to omit.

## Interaction with clippy and sibling lints

- **`clippy::needless_collect` does not cover this.** That lint flags an
  intermediate `collect` consumed immediately by `.len()`,
  `.is_empty()`, `.contains()`, `.iter()`, `.into_iter()`, and similar —
  consumers whose work the iterator could do directly *without* any
  extra crate. It deliberately omits `.join()`, because the
  allocation-free fix is not in `std` (the iterator has no `join`); it
  lives in `itertools`. `perfectionist::collect_then_join` is therefore
  a **complement**, not a refinement, of `clippy::needless_collect`, and
  the two never fire on the same expression — which is why this rule
  does **not** borrow the `needless_collect` name (per the
  [naming convention](./IMPLEMENTATION_CONVENTIONS.md#mirror-the-clippy-name-only-for-a-genuine-refinement)).
- **Slice-concatenating `join` is out of scope.**
  `vecs.collect::<Vec<_>>().join(&sep_slice)` producing a `Vec<T>` has
  no `itertools` equivalent; step 1's result-type check excludes it.
- **`.collect::<Vec<_>>().concat()` is out of scope.** The separator-less
  concatenation has its own remedies (`.collect::<String>()` for the
  string case, with no `itertools` needed), a distinct trigger and a
  distinct fix; bundling it here would merge two anti-patterns under one
  banner (see the
  [one-rule-per-file convention](../CLAUDE.md#one-rule-per-file-one-config-per-rule)).
  If worth catching it belongs in a sibling rule.
- **Bound-variable join is out of scope.** A `let v: Vec<String> = …;`
  followed later by `v.join(sep)` is not a direct chain; flagging it
  would require the cross-statement use-analysis `clippy::needless_collect`
  performs, which this rule deliberately avoids. Only the direct
  `collect().join()` chain fires.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
