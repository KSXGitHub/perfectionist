# Cross-`cfg` undefined identifiers: `unresolved_path`, `unresolved_import`

**Source:** project convention, closing a structural blind spot in
rustc's name resolution. This file groups the cross-`cfg` checks for
references to names that **do not exist in some configuration the
reference is compiled in**. They share the mechanism and the
simple-enough-`cfg` gate of the other two `cfg` rules
([`overly_complex_cfg`](./overly-complex-cfg.md),
[`cross-cfg-dead-code`](./cross-cfg-dead-code.md)), so they live
together; at implementation each is its own rule and `Config`.

## The gap

`#[cfg]`-disabled code is stripped during macro expansion, **before**
name resolution. So a reference to an identifier that exists only under
one `cfg`, used under a *different* `cfg`, is a latent
`error[E0425]` / `E0412` / `E0433` (cannot find value / type / module)
or `E0432` (unresolved import) — but the compiler **never checks it**
unless you build the exact configuration that strips the definition
while keeping the use. A typo, a deleted item, or a feature gate that
drifted out of sync sits undetected until some user builds that
combination.

```rust
#[cfg(feature = "fast")]
fn helper() -> u32 { 1 }

#[cfg(feature = "logging")]   // independent of `fast`
fn log_it() { let _ = helper(); }   // E0425 when `logging` && !`fast`
```

`cargo check` with both features, or with neither, is clean. Only the
`logging && !fast` build fails — and only if CI happens to try it.

## Naming: there is no rustc/Clippy *lint* to mirror

Undefined identifiers are **hard errors** (`E0425`, `E0412`, `E0433`,
`E0432`), not warn-level lints, so there is no existing lint *name* to
borrow. The names here mirror rustc's **error terminology**
("unresolved import", "failed to resolve") while obeying the
anti-pattern-naming convention: `unresolved_path` and `unresolved_import`
name the offending construct, read correctly under `#[allow(...)]`, and
claim no more than the trigger checks. They are *not* presented as
refinements of a Clippy lint, because none exists.

## Scope: `cfg`-induced unresolvability only

**These rules fire only when a reference is unresolvable in some
configuration *because of* a `cfg` mismatch. A reference that is
unresolvable in the configuration being built is rustc's job — it is
already a hard error — and is left entirely to the compiler.** The
trigger is: a reference with `cfg`-context `R` resolves (heuristically)
to a set of candidate definitions with contexts `{D_i}`, and `R ∧
¬(⋁ D_i)` is satisfiable — there is a configuration that compiles the
reference but none of its candidate definitions. This is the implication
`R ⊨ ⋁ D_i` *failing*, which is why the analysis needs the satisfiability
machinery (see [`overly_complex_cfg`](./overly-complex-cfg.md)'s *The
SAT connection*) and the simple-enough gate.

## Why is this bad?

This is **not** a stylistic preference — it is an objective latent
defect. A reference that fails to resolve in a reachable configuration
is a build that *will not compile* for whoever selects that
configuration. The only reason it is not already a compile error for you
is that your build does not happen to be that configuration. Surfacing
it at lint time, against every simple-enough `cfg` combination at once,
catches the broken build before a downstream user (a different platform,
a different feature set) hits it — exactly the "do I have to spin a CI
matrix to be sure?" problem, answered statically for the tractable
cases.

## The two sub-lints

### `perfectionist::unresolved_path`

A path expression / type / macro-or-trait path reference whose
`cfg`-context `R` admits a configuration where the reference compiles
but no candidate definition of that name does (`R ∧ ¬(⋁ D_i)`
satisfiable). Flag the reference, naming the witnessing configuration
(*"`helper` is not defined when `feature = "logging"` is on and
`feature = "fast"` is off"*). Covers the `E0425` / `E0412` / `E0433`
families.

### `perfectionist::unresolved_import`

The same, specialised to `use` items: a `use some::path::Item;` whose
target `Item` (or an intermediate module segment) is absent in a
configuration the `use` itself is compiled in. The cross-`cfg` analogue
of `E0432`.

## Examples

**Avoid:** definition and use behind independent feature gates.

```rust
#[cfg(feature = "fast")]
mod fast { pub fn helper() -> u32 { 1 } }

#[cfg(feature = "logging")]
fn log_it() { let _ = fast::helper(); }   // unresolved when logging && !fast
```

`perfectionist::unresolved_path` flags `fast::helper`, witness: `feature
= "logging"` on, `feature = "fast"` off.

**Not flagged:** the use's gate implies the definition's gate, so every
config that compiles the use also compiles the definition.

```rust
#[cfg(feature = "fast")]
fn helper() -> u32 { 1 }

#[cfg(all(feature = "fast", feature = "logging"))]   // implies `fast`
fn log_it() { let _ = helper(); }
```

Here `R = fast ∧ logging` implies `D = fast`, so `R ∧ ¬D` is
unsatisfiable — no witnessing config — and the rule stays silent.

**Skipped (too complex):** if the combined `cfg` exceeds the
analyzability budget, the rule declines;
[`overly_complex_cfg`](./overly-complex-cfg.md) flags the predicate.

## Configuration

```toml
[perfectionist::unresolved_path]
max_analyzable_atoms = 4
max_analyzable_depth = 3
```

Same analyzability-gate fields as the dead-code rules, defaulting to the
shared budget. `unresolved_import` carries the same two fields in its own
`Config`.

## Implementation notes

- **`LateLintPass` driven by `src/module_reparse.rs`**, for the same
  reason as the sibling rules: it must see the written,
  all-`cfg`-variants set of definitions *and* references across every
  module, which only a re-parse in a late pass gives (a pre-expansion
  pass misses separate-file submodules; a post-expansion pass has the
  `cfg`-disabled code stripped). See the
  [source-layout-rules section](./IMPLEMENTATION_CONVENTIONS.md#reaching-every-module-source-layout-rules).

- **The hard part is candidate resolution, and it must over-approximate
  the *definition* side.** Soundness here is the opposite bias from the
  dead-code rules: to avoid a false "undefined", treat the set of
  candidate definitions `{D_i}` as *generously* as possible — every
  item, import, glob target, and prelude name that could plausibly
  provide the name in *any* config. Only flag when, even with that
  generous candidate set, there is still a satisfiable config covering
  none of them. Concretely, skip (do not flag) when:
  - the name could come through a **glob import** (`use foo::*;`) whose
    contents cannot be enumerated pre-resolution,
  - the name could be **macro-generated**,
  - the reference is a **method / associated item** resolved by type
    (the rule reasons about path resolution, not trait selection), or
  - any candidate's own `cfg` is over the analyzability budget.
  This keeps false positives near zero at the cost of missing some real
  breaks — the right trade, since a false "this doesn't compile" on code
  that does is the most annoying possible diagnostic.

- **Use the shared `src/cfg_analysis.rs` oracle** for the `R ∧
  ¬(⋁ D_i)` satisfiability query and the domain constraints. Do not
  re-derive it.

- **No autofix.** The fix (widen the definition's gate, narrow the use's
  gate, or fix a typo) is a judgement call; emit a diagnostic with the
  witnessing configuration and stop. The witness *is* the actionable
  output.

- **Diagnostic anchoring.** The reference usually has a live span via
  the re-parse's shared `SourceMap`; anchor there. A reference inside a
  currently-`cfg`-disabled region has real source coordinates but may
  lack a live HIR node, so a local `#[allow]` may not resolve — note
  this where it applies.

- See
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md) for
  cross-cutting conventions, in particular `perfectionist::*` lint-name
  namespacing.

### Difficulty

**Hard** — the hardest of the `cfg` cluster. The satisfiability query is
mechanical given `src/cfg_analysis.rs`, but the generous,
pre-resolution candidate-gathering is delicate: too narrow and the rule
false-positives on resolvable code, too broad and it never fires.
`unresolved_import` (explicit `use` targets) is the more tractable of the
two and a good first deliverable; full path-expression `unresolved_path`
is the stretch.

## Interaction with sibling rules

- [`cross-cfg-dead-code`](./cross-cfg-dead-code.md) — the dual. Both
  build a `cfg`-context per definition and per reference and run a
  bounded satisfiability query; dead-code asks whether a *present*
  definition goes *unused* in some config (`D ∧ ¬(⋁ use_cfg)`), this
  asks whether a *needed* definition is *absent* in some config (`R ∧
  ¬(⋁ def_cfg)`). They should share the context-collection code in
  `src/cfg_analysis.rs`.
- [`overly_complex_cfg`](./overly-complex-cfg.md) — owns the shared
  measure/oracle and flags the predicates this rule skips.

## Interaction with stock lints

rustc resolves names only in the configuration being compiled, where an
undefined identifier is a hard error these rules defer to entirely.
Nothing in rustc or Clippy checks resolvability *across* configurations:
`#[cfg]`-stripped code is gone before resolution runs, and
`unexpected_cfgs` (check-cfg, [RFC 3013](https://rust-lang.github.io/rfcs/3013-conditional-compilation-checking.html))
validates only `cfg` atom names/values, never the identifiers inside
`cfg`-gated code. This is the unfilled gap these rules occupy.

## Default state

**Active by default**, for both sub-lints. Active-by-default is safe
because the analysis is deliberately one-sided: it gathers candidate
definitions *generously* (every item, import, glob target, and prelude
name that could provide the name in any config) and flags only when even
that generous set leaves a satisfiable config uncovered, skipping
entirely whenever resolution is uncertain (see *Implementation notes*).
That keeps a false "this won't compile" — the costliest possible
diagnostic — near zero. A residual false positive is suppressed at the
site with `#[allow(perfectionist::unresolved_path)]` (or
`unresolved_import`) and a reason.
