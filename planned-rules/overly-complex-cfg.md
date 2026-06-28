# `overly_complex_cfg`

**Source:** project convention. `#[cfg(...)]` predicates accrete
operators over a crate's life — a platform check picks up a feature
gate, then a `not(...)`, then a nested `any(all(...), ...)` — until no
reader (and no tool) can confidently say which configurations compile
the item. This rule caps that growth.

It is also the **home of the shared cfg-complexity measurement** that
the two analysis rules
[`cross-cfg-dead-code`](./cross-cfg-dead-code.md) and
[`cross-cfg-unresolved-path`](./cross-cfg-unresolved-path.md) consult to
decide whether a given `#[cfg]` is *simple enough* to reason about. The
measurement is defined once here; see *The shared complexity measure*
below.

## Statement

A `#[cfg(...)]` / `#[cfg_attr(...)]` predicate (and the `cfg!(...)`
macro) should stay within a small structural budget: few distinct
atoms, shallow nesting, and no negation wrapped around a compound
sub-expression. A predicate that exceeds the budget is flagged; the
author either simplifies it (factor a named intermediate, hoist a
common term, introduce a `feature` that bundles the conditions) or
suppresses the rule with a reason.

## Why restrict this?

This is a stylistic preference, not a correctness issue: a complex
`#[cfg]` predicate compiles and selects exactly the configurations its
boolean structure says it does. The objection is twofold.

- **A human cannot evaluate it by inspection.** `all(unix,
  any(feature = "a", not(all(feature = "b", target_endian =
  "big"))))` forces the reader to run a truth table in their head to
  answer "does this compile on my machine?". `#[cfg]` is read far more
  often than written — on every "why isn't this code active?"
  investigation — and a predicate no one can read is re-derived wrong.

- **A tool cannot evaluate it cheaply either, and that is not a
  coincidence.** Deciding the questions the sibling rules ask —
  *is this item ever used in a configuration where it exists?*
  (dead code), *is every name this reference needs present whenever the
  reference compiles?* (unresolved path) — reduces to boolean
  **satisfiability / implication** over the cfg atoms. Those problems
  are NP-/co-NP-hard in general (see *The SAT connection*), so the
  analysis is only sound and fast when the predicate has few distinct
  atoms. **The complexity that defeats a human reader is the same
  complexity that defeats the analysis**, which is why the default
  budget for this rule is set at the analyzability boundary the sibling
  rules use: a predicate this rule flags is, by default, exactly one the
  sibling rules will decline to check. Flagging it says *"no tool can
  verify this `#[cfg]` for you — simplify it, or fall back to a CI
  matrix that actually builds the configuration."*

## The shared complexity measure

The single source of truth for "how complex is this `#[cfg]`?", used by
this rule's trigger **and** by the sibling rules' simple-enough gate.
Implemented once as a crate-internal helper (see *Implementation
notes*) and consumed everywhere a cfg predicate's tractability matters.

A cfg predicate is the obvious recursive tree: leaves are **atoms**
(`unix`, `test`, `feature = "x"`, `target_os = "linux"`, …) and
internal nodes are `all(..)`, `any(..)`, `not(..)`. The measure reports:

- **`distinct_atoms`** — the number of *distinct* atoms (`all(unix,
  feature = "x", unix)` has two). This is the dominant metric: a sound
  decision procedure enumerates the `2^distinct_atoms` truth assignments
  (intersected with the domain constraints below), so it is `distinct`,
  not total, occurrences that bound the cost.
- **`depth`** — maximum nesting depth of `all`/`any`/`not`.
- **`negation_over_compound`** — whether any `not(..)` wraps a
  non-leaf. `not(unix)` is a benign literal; `not(all(a, b))` is a De
  Morgan expansion that doubles the clause count and is the single
  biggest readability and reasoning hazard.
- **`total_terms`** — total node count, a tie-breaker for predicates
  that are wide but shallow.

The two senses of "too complex" share this measure but apply different
thresholds, because they answer different questions:

- **Readability budget** (this rule's trigger). Configurable; see
  *Configuration*.
- **Analyzability budget** (the sibling rules' gate). Each sibling rule
  owns its own threshold field (per the one-`Config`-per-rule
  convention) but they default to the same values and call the same
  measure. A predicate at or under the analyzability budget is decided
  exactly by truth-table enumeration in microseconds; above it, the
  sibling rule skips the predicate rather than risk an exponential walk
  or an unsound guess.

### Domain constraints on cfg atoms

cfg atoms are **not** independent boolean variables, and any decision
procedure built on this measure must respect that or it will both miss
violations and manufacture false ones:

- `target_os` (and `target_arch`, `target_env`, `target_endian`, …) is
  **single-valued**: `all(target_os = "linux", target_os = "windows")`
  is unsatisfiable, though pure boolean SAT calls it satisfiable.
- Some atoms **imply** others: `target_os = "linux"` implies `unix` and
  `target_family = "unix"`; `target_os = "windows"` implies `windows`.

The bounded enumeration intersects the `2^n` assignments with these
constraints (a small, fixed theory of the well-known cfg keys — closer
to SMT-modulo-a-theory than raw SAT). Unknown / user-defined atoms
(`feature = "..."`, arbitrary `cfg(my_flag)`) are treated as free and
independent. The helper exposes this as a conservative oracle: when a
predicate mixes known-constrained atoms it cannot fully model, it
reports *unknown* and the sibling rules treat that like "too complex"
and skip.

## The SAT connection

Worth recording because it justifies the whole "simple enough" framing,
and because the abstract version of the question (does nesting
`and`/`or`/`not` "trigger BOOL SAT"?) answers a subtly *different*
problem than the one these rules face:

- For **one predicate in isolation**, "is it satisfiable?" is trivial
  when there is no negation (set every atom true — a monotone formula is
  always satisfiable) and NP-complete once `not` is freely combined with
  `any`/`all` (`{and, not}` and `{or, not}` are each functionally
  complete via De Morgan).
- But the sibling rules never ask bare satisfiability of a single
  predicate. They ask **implication**: does `use_cfg` imply `def_cfg`?
  That is `unsat(use_cfg ∧ ¬def_cfg)` — and the `¬` reintroduces
  negation **even when both predicates were written with only
  `all`/`any`**. So the "monotone is easy" escape hatch does not apply:
  the query is the hard direction regardless of how the source `#[cfg]`s
  look.

The practical consequence is the design choice above: rather than gate
on *which operators appear* (the textbook lens), gate on the **distinct
atom count** (the truth-table size). With few atoms, every question —
satisfiability, implication, mutual exclusion — is decided exactly by
enumeration; with many, all of them are intractable at once. This is why
`distinct_atoms` is the dominant metric in the measure.

## What to lint

For every `#[cfg(P)]`, `#[cfg_attr(P, ...)]`, and `cfg!(P)` in the
crate, compute the measure of `P` and flag `P` when it exceeds the
configured readability budget — any one of:

- `distinct_atoms > max_distinct_atoms`, or
- `depth > max_depth`, or
- `negation_over_compound` is set and `forbid_negation_over_compound`
  is on, or
- `total_terms > max_total_terms`.

The diagnostic names which bound was exceeded and suggests the
remedy that fits (hoist a common term out of an `any`/`all`; replace a
`not(all(..))`/`not(any(..))` with its De Morgan dual if that lowers the
count; introduce a named `feature` that bundles a recurring
sub-predicate).

### What's *not* in scope

- **Reducible-but-simple shapes** like `any(unix)` or `all()` —
  `clippy::non_minimal_cfg` already unwraps single-element `any`/`all`
  and double negation. This rule is about *size*, not about trivially
  removable wrappers; it defers the minimal-form cleanup to Clippy and
  counts the cfg *after* such trivial reductions.
- **`#[cfg(predicate)]` on the `Cargo.toml` side** (target tables) —
  not source, not reachable by a lint.

## Examples

**Avoid:** five distinct atoms and a negated compound — unreadable, and
beyond what the sibling rules can verify.

```rust
#[cfg(all(
    unix,
    any(feature = "tls", feature = "native-tls"),
    not(all(target_endian = "big", target_pointer_width = "32"))
))]
fn connect() { /* ... */ }
```

**Prefer:** name the recurring condition once, keep each `#[cfg]` small.

```rust
// In Cargo.toml: `any-tls = ["tls", "native-tls"]`
#[cfg(all(unix, feature = "any-tls", not(feature = "legacy-be32")))]
fn connect() { /* ... */ }
```

**Not flagged:** within budget (two distinct atoms, depth 2, a negated
*literal*, not a negated compound).

```rust
#[cfg(all(unix, not(feature = "no-net")))]
fn connect() { /* ... */ }
```

## Configuration

```toml
[perfectionist::overly_complex_cfg]
max_distinct_atoms = 4
max_depth = 3
max_total_terms = 12
forbid_negation_over_compound = false
```

- `max_distinct_atoms` — flag a predicate with more than this many
  distinct atoms. Defaults to `4` (≤ 16 truth-table rows — the point
  past which the sibling rules also stop). The dominant knob.
- `max_depth` — flag a predicate nested deeper than this. Defaults to
  `3`.
- `max_total_terms` — flag a predicate with more than this many total
  nodes, catching wide-but-shallow predicates the atom/depth caps miss.
  Defaults to `12`.
- `forbid_negation_over_compound` — when `true`, flag any `not(..)`
  wrapping a compound regardless of the size caps, since the De Morgan
  expansion is the worst readability offender. Defaults to `false`
  (the size caps usually catch these anyway).

## Implementation notes

- **Pre-expansion token / AST scan, but reaching every module.** The
  rule reads the *written* `#[cfg]` predicates, which only exist before
  cfg-stripping. As with the import-layout rules, the naïve
  pre-expansion `EarlyLintPass` walk silently skips every separate-file
  `mod foo;` (still `ModKind::Unloaded` pre-expansion), so this rule
  must run as a `LateLintPass` that **re-parses the crate's module
  files** through [`src/module_reparse.rs`](../src/module_reparse.rs);
  re-parsing keeps `#[cfg]` intact *and* reaches every file. Read the
  "Reaching every module (source-layout rules)" section of
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#reaching-every-module-source-layout-rules)
  before starting — this rule is squarely in that category. Unlike the
  import rules, it does *not* discard cfg-disabled inline modules: a
  `#[cfg]` inside a currently-disabled module is still a predicate worth
  measuring. Anchor each diagnostic at the attribute's own span (which a
  shared `SourceMap` re-parse preserves); where the attribute sits in a
  cfg-disabled region with no live HIR node, the span still points at
  real source though a local `#[expect]` may not resolve — note this in
  the diagnostic.

- **The cfg parser is a `take_*` combinator set, not a regex.** Parsing
  `all`/`any`/`not`/atom is exactly the small fixed grammar the
  "Parser style" section of
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#parser-style)
  prescribes hand-rolled combinators for — though in practice the
  predicate is available as a parsed `MetaItem` tree from the attribute,
  so the "parser" is a tree walk rather than a string scan.

- **Factor the measure into a crate-internal module.** This rule is the
  *first* of the cfg cluster; per the cross-rule-helper convention it
  owns extraction of the shared helper. Suggested home:
  `src/cfg_analysis.rs` (`pub(crate)`), holding the cfg-predicate
  representation, the `cfg_complexity()` measure defined above, the
  domain-constraint table, and the bounded decision oracle the sibling
  rules call. Do not duplicate any of this in the sibling rules.

- See
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for the cross-cutting conventions every rule follows, in particular
  the `perfectionist::*` lint-name namespacing.

### Difficulty

**Easy–Medium.** The measure and the trigger are a straightforward tree
walk over a parsed `MetaItem`. The medium part is the module-reparse
plumbing (shared with the sibling rules) and deciding the diagnostic
anchor for predicates in cfg-disabled regions.

## Interaction with sibling rules

- [`cross-cfg-dead-code`](./cross-cfg-dead-code.md) and
  [`cross-cfg-unresolved-path`](./cross-cfg-unresolved-path.md) consume
  the measure defined here through their own analyzability thresholds.
  When all three are active, a predicate over budget is flagged by *this*
  rule and skipped (not silently mis-analyzed) by the other two — a
  deliberately coherent split: this rule tells you the `#[cfg]` is
  unverifiable, the others verify only what is left.

## Interaction with stock lints

- `clippy::non_minimal_cfg` removes trivially redundant `any()`/`all()`
  wrappers and double negation. It is complementary: it makes a
  predicate *minimal*, this rule bounds a minimal predicate's *size*.
  Run both; this rule measures the post-minimisation shape.
- `unexpected_cfgs` (rustc check-cfg) validates atom *names/values*
  against an expected set. Orthogonal: it never looks at the boolean
  structure this rule measures.

## Default state

Active by default. The default budget is permissive and set at the
analyzability boundary, so an out-of-the-box run flags only predicates
that are simultaneously unreadable and unverifiable. Projects with a
stricter house style lower the caps; a project that genuinely needs one
gnarly predicate suppresses it at the site with
`#[expect(perfectionist::overly_complex_cfg)]` and a reason.
