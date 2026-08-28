# Cross-`cfg` dead code: `dead_code`, `unused_imports`, `unused_variables`

**Source:** project convention, closing a structural gap in rustc's own
warnings. This file groups the cross-`cfg` counterparts of three
warn-by-default rustc lints. They share one mechanism (collect each
item's and each use's `cfg`-context, then decide reachability across
configurations) and one gate (analyse only `#[cfg]`s simple enough to
decide soundly), so they live together; at implementation each is its
own rule and `Config` per the one-rule-per-file convention.

## The gap

rustc's `dead_code`, `unused_imports`, and `unused_variables` all run
*after* `#[cfg]` stripping — `#[cfg]` is resolved during macro
expansion, before name resolution and analysis — so each compilation
sees exactly **one** configuration. Consequences:

- An item used **only** under `#[cfg(feature = "x")]` is reported dead
  when you build without `x`, and not examined for deadness at all when
  you build *with* `x` (it is used there). Whether it is dead in some
  *other* configuration is never asked.
- The reverse leftover — an item gated `#[cfg(feature = "x")]` whose
  every use was deleted or itself moved behind a *different* gate — is
  invisible in any single build: the config that compiles the item does
  not compile a use, but no single `cargo check` exercises that config
  against the dead-detection pass.

Verifying this by hand means a CI matrix over every feature combination
and target. These rules do statically what that matrix does
dynamically, for the `#[cfg]` shapes simple enough to decide.

## Scope: `cfg`-induced deadness only

**These rules fire only on deadness that arises from a `cfg` mismatch
between an item's definition and its uses. Everything else is left to
rustc.** Rationale:

- Within one active configuration, rustc's `dead_code` /
  `unused_imports` / `unused_variables` are authoritative and have more
  information than any Dylint pass. Re-checking the in-config cases would
  be a strictly worse duplicate and would double-fire.
- The value these rules add is exactly the configurations the current
  build does not exercise. So the trigger is specifically: there is a
  satisfiable configuration in which the **definition is compiled but no
  use is** — and that configuration differs from the one being built (or
  the item/use `cfg`-contexts differ at all).
- The mechanism enforces the scope anyway: seeing other configurations
  requires reasoning symbolically over `cfg` expressions across all
  variants; a definition and its uses sharing identical (or absent)
  `cfg` are already fully handled by rustc post-expansion.

So: fire on a `cfg` **mismatch** whose predicates are **simple enough**
to decide; skip complex `cfg` (defer to the CI matrix — and to
[`overly_complex_cfg`](./overly-complex-cfg.md), which flags it); skip
no-`cfg` and identical-`cfg` cases (defer to rustc).

## Why is this bad?

This is largely a hygiene preference for ordinary dead code — and for
that part the disclaimer applies: dead code compiles and runs fine; the
objection is to the cruft and the maintenance drag. But the cross-`cfg`
framing adds an objective edge the single-config lints cannot reach: a
definition that is dead **in a configuration you ship but do not build
in CI** is a genuine latent defect — most often a feature that was
renamed or removed in `Cargo.toml` while its `#[cfg(feature = "...")]`
code was left behind, now permanently unreachable and undetectable by a
normal `cargo check`. rustc structurally cannot warn here (it never
compiles that configuration against the dead-code pass); the value is
surfacing the leftover before it rots for years.

## The three sub-lints

Each mirrors its rustc namesake (same anti-pattern, broadened to the
configurations rustc cannot see) and registers under the
`perfectionist::` namespace, so there is no collision with the built-in
lint. The mirrored name is deliberate per the "mirror the Clippy name
only for a genuine refinement" convention: same anti-pattern (dead /
unused), broader trigger (cross-`cfg`), not an orthogonal or opposite
concern. Each diagnostic states plainly that it fires on the cross-`cfg`
case rustc misses, so a reader does not mistake it for a re-run of the
built-in.

### `perfectionist::dead_code`

An item (`fn`, `struct`, `enum`, `const`, `impl` method, …) whose
definition `cfg`-context `D` is satisfiable, but for which there is a
satisfiable configuration under `D` where **none** of its uses' contexts
hold — i.e. `D ∧ ¬(⋁ use_cfg_i)` is satisfiable. Flag the item, naming
the configuration witnessing the deadness (e.g. *"unused when `feature =
"x"` is enabled and `unix` is false"*).

### `perfectionist::unused_imports`

The same analysis for `use` items: a `use` whose `cfg`-context admits a
configuration in which the imported name is never referenced. This is
the cross-`cfg` analogue of the lingering import — distinct from
[`private_reexport_imports`](./private-reexport-imports.md), which is
about *which binding* an import traverses, not about a `cfg`-config in
which it goes unused.

### `perfectionist::unused_variables`

The same analysis for `let` bindings whose initializer or later uses sit
behind a `cfg` that can be disabled independently of the binding (e.g. a
binding compiled unconditionally but read only inside a
`#[cfg(feature = "x")]` block). Narrower in practice — most local
deadness is intra-config and already rustc's job — so this sub-lint is
the most conservative of the three and the readiest to ship last.

## Examples

**Avoid:** helper compiled in two configs, used in only one.

```rust
// `parse_fast` exists whenever `simd` OR `fallback` is on,
// but is *called* only under `simd`.
#[cfg(any(feature = "simd", feature = "fallback"))]
fn parse_fast(input: &[u8]) -> u32 { /* ... */ }

#[cfg(feature = "simd")]
fn run(input: &[u8]) -> u32 { parse_fast(input) }
// Build with `fallback` but not `simd`: `parse_fast` is dead,
// and no normal `cargo check` of that config reports it.
```

`perfectionist::dead_code` flags `parse_fast`, witness: `feature =
"fallback"` on, `feature = "simd"` off.

**Not flagged:** definition and uses share the same gate (rustc's job in
the `simd` build; trivially live there).

```rust
#[cfg(feature = "simd")]
fn parse_fast(input: &[u8]) -> u32 { /* ... */ }

#[cfg(feature = "simd")]
fn run(input: &[u8]) -> u32 { parse_fast(input) }
```

**Skipped (too complex):** the deciding `cfg` exceeds the analyzability
budget, so the rule declines rather than guesses —
[`overly_complex_cfg`](./overly-complex-cfg.md) flags the predicate
instead.

## Configuration

Each sub-lint carries its own `Config` (one per rule). Common fields:

```toml
["perfectionist::dead_code"]
max_analyzable_atoms = 4
max_analyzable_depth = 3
```

- `max_analyzable_atoms` / `max_analyzable_depth` — the analyzability
  gate from [`overly_complex_cfg`](./overly-complex-cfg.md)'s shared
  measure. A definition/use cluster whose combined `cfg` exceeds either
  bound is skipped. Defaults match the complexity rule's budget (`4` /
  `3`) so "what this rule checks" and "what the complexity rule flags"
  partition cleanly.

`unused_imports` and `unused_variables` carry the same two fields.

## Implementation notes

- **`LateLintPass` driven by `src/module_reparse.rs`.** These read the
  written, all-`cfg`-variants layout of items and uses across every
  module, so they are exactly the "source-layout rule" shape of
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#reaching-every-module-source-layout-rules):
  a pre-expansion `EarlyLintPass` would miss separate-file submodules
  and a post-expansion pass would have the `cfg`-disabled code already
  stripped. Re-parse to get both reach and `cfg`-preservation. Like
  [`overly_complex_cfg`](./overly-complex-cfg.md) and unlike the import
  rules, do *not* drop `cfg`-disabled inline modules — their items and
  uses are part of the cross-config picture.

- **Use the shared `src/cfg_analysis.rs` oracle.**
  [`overly_complex_cfg`](./overly-complex-cfg.md) owns the
  cfg-representation, the complexity measure, the domain-constraint
  table (single-valued `target_os`, `unix`/`windows` implications, …),
  and the bounded `2^n` decision procedure. These rules call the oracle
  with the constructed `D ∧ ¬(⋁ use_cfg)` query; they do not re-derive
  satisfiability logic. When the oracle reports *unknown* (atoms it
  cannot model, or over budget), skip.

- **Building the `cfg`-context.** An item's / use's context is the
  conjunction of every enclosing `#[cfg]` — on the item, its `impl`, its
  module chain, and any `cfg_attr`-applied attribute. Computing it from
  the re-parsed AST is the bulk of the per-rule work.

- **Name resolution is heuristic pre-resolution.** Matching a use to the
  definitions it could bind, across configs, cannot use the real
  resolver (it ran on one config, post-strip). Match by name/path
  conservatively and **only flag when confident**: glob imports,
  macro-generated names, and trait-method resolution defeat exact
  matching, so treat an unresolved-but-plausible match as a *use*
  (suppressing the dead-code finding) rather than risk a false positive.
  The autofix is "delete the item," so a false positive deletes live
  code — bias hard toward silence.

- **No autofix beyond a `MaybeIncorrect` suggestion.** Suggest removal
  (or moving the item under the use's gate) only as a non-machine-
  applicable hint, given the heuristic resolution.

- **Proc-macro suppression.** Apply the standard guard if a flagged
  item/use can be derive-synthesised with a user-source span; add a
  regression fixture per the convention's
  [proc-macro section](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).

- See
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md) for
  cross-cutting conventions, in particular `perfectionist::*`
  lint-name namespacing.

### Difficulty

**Hard.** The `cfg`-context collection and the bounded decision are
mechanical once `src/cfg_analysis.rs` exists, but the cross-config,
pre-resolution name matching is the wedge: it must be conservative
enough that the "delete this" suggestion never targets live code.
`dead_code` and `unused_imports` are the tractable pair; `unused_variables`
is the most error-prone and may ship last.

## Interaction with sibling rules

- [`overly_complex_cfg`](./overly-complex-cfg.md) — owns the shared
  measure and oracle these rules call, and flags the predicates these
  rules skip. The default thresholds align so the two partition the
  space of `#[cfg]`s.
- [`cross-cfg-unresolved-path`](./cross-cfg-unresolved-path.md) — the
  dual: it asks "is a needed definition *absent* in some config the
  reference compiles in?", this asks "is a present definition *unused*
  in some config it compiles in?". Same machinery, opposite direction
  of the implication.
- [`private_reexport_imports`](./private-reexport-imports.md) — also
  about imports `unused_imports` cannot retire, but on the
  *ancestor-privilege* axis, not the `cfg`-config axis. No overlap.

## Interaction with stock lints

rustc's `dead_code` / `unused_imports` / `unused_variables` cover the
in-config cases (which these rules deliberately skip). There is no
existing lint for the cross-`cfg` cases — confirmed by
[RFC 3013](https://rust-lang.github.io/rfcs/3013-conditional-compilation-checking.html),
whose check-cfg work validates `cfg` names/values but never reasons
about cross-config reachability. These rules and the built-ins are
strictly complementary.

## Default state

**Active by default**, for all three sub-lints. Active-by-default is
safe here precisely because the analysis is built to be conservative:
the cross-config name matching treats any uncertain match (glob imports,
macro-generated names, type-resolved methods) as a *use*, so the rules
stay silent unless the deadness witness is unambiguous (see
*Implementation notes*). The suggested removal is a non-machine-
applicable hint, never an auto-applied edit. A residual false positive is
suppressed at the site with `#[expect(perfectionist::dead_code)]` (or the
per-sub-lint name) and a reason.
