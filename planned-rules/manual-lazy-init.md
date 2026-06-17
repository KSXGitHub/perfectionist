# `manual_lazy_init`

**Source:** project convention, motivated by
[`pnpm/pnpm#12477`](https://github.com/pnpm/pnpm/pull/12477), which
migrated the eligible `OnceLock` sites in that codebase to `LazyLock`
and left the ineligible ones alone. The same anti-pattern exists one
thread-safety tier down (`OnceCell` → `LazyCell`), so this rule covers
both std pairs.

## Statement

A `Once*` cell that is only ever read through a `get_or_init` call with
one **fixed, non-capturing** initializer is a hand-rolled `Lazy*`. The
`Lazy*` type bakes that initializer into the cell, so every read becomes
a plain deref and the `get_or_init` boilerplate disappears. There are
exactly two such pairs in std, and the rule treats them uniformly:

| Anti-pattern (`Once*`)   | Suggested (`Lazy*`)      | Tier                      | Where it lives           |
|--------------------------|--------------------------|---------------------------|--------------------------|
| `std::sync::OnceLock<T>` | `std::sync::LazyLock<T>` | thread-safe (`Sync`)      | a `static` / `const`     |
| `std::cell::OnceCell<T>` | `std::cell::LazyCell<T>` | single-threaded (`!Sync`) | a `thread_local!` static |

```rust
// Avoid: OnceLock used purely as a lazy-initialised global.
static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
    REGISTRY.get_or_init(|| Registry::load_builtin())
}

// Prefer: the initializer lives on the cell.
static REGISTRY: LazyLock<Registry> = LazyLock::new(|| Registry::load_builtin());

fn registry() -> &'static Registry {
    &REGISTRY
}
```

The single-threaded case is identical in spirit, in a `thread_local!`:

```rust
// Avoid:
thread_local! {
    static SCRATCH: OnceCell<Scratch> = OnceCell::new();
}
SCRATCH.with(|c| c.get_or_init(Scratch::new).use_it());

// Prefer:
thread_local! {
    static SCRATCH: LazyCell<Scratch> = LazyCell::new(Scratch::new);
}
SCRATCH.with(|c| c.use_it());
```

The lint flags the `Once*` declaration (and offers to rewrite both the
declaration and its `get_or_init` call sites) when, and only when, the
cell's entire usage is expressible as the matching `Lazy*`. The two
sections below — "What makes a `Once*` replaceable" and "When `Lazy*`
cannot replace `Once*`" — are the trigger predicate and its complement;
both are load-bearing, because the value of the rule is in *not* firing
on the second set.

### Why one rule, not two (`manual_lazy_lock` + `manual_lazy_cell`)

The `OnceLock`→`LazyLock` and `OnceCell`→`LazyCell` checks share one
trigger predicate (parameterised only by a two-row type table), have no
configuration, emit the same diagnostic shape, and carry the same
difficulty and MSRV (both `Lazy*` types stabilised in Rust 1.80). Per
`CLAUDE.md` ("One rule per file"), a split is warranted only when
sub-checks have *distinct predicates, disjoint config, and no shared
diagnostic* — lock-vs-cell has none of those, so they are one rule. The
name `manual_lazy_init` is deliberately tier-agnostic so it claims no
more than it checks; `manual_lazy_lock` would over-claim the moment it
fired on a cell. (Contrast `manual_derive_more_impl`, whose multiple
sub-lints *are* justified: its per-trait checks differ in AST pattern,
difficulty, and opt-in maturity.)

## Why restrict this?

This is a stylistic preference, not a correctness issue. A
`get_or_init`-only `Once*` and the equivalent `Lazy*` behave
identically: both initialise lazily on first access, both run the
initializer exactly once, each within its thread-safety tier. Nothing is
broken by keeping the `Once*`.

The project prefers `Lazy*` for the eligible shape because:

- **The initializer is attached to the value it initialises.** A reader
  who lands on the declaration sees *what* it will hold without chasing
  every `get_or_init` call site to confirm they all pass the same
  closure. With `Once*` the initializer is a property of the call site,
  and nothing in the language stops two call sites from racing with
  *different* closures — a latent footgun the `Lazy*` form removes by
  construction.
- **Reads stop carrying boilerplate.** `&REGISTRY` replaces
  `REGISTRY.get_or_init(|| ...)` at every use, and the closure is
  written once rather than repeated (or hidden behind an accessor
  function that exists only to host it).
- **The type states the intent.** `Lazy*<T>` says "lazily computed,
  then immutable". `Once*<T>` says "written once, *somewhere, somehow*"
  — a strictly weaker claim that invites `set`-from-outside patterns the
  eligible sites never use.

## What makes a `Once*` replaceable

All of the following must hold for the cell:

1. **It lives where the `Lazy*` type can.** For `OnceLock`: a `static`
   or `const` item — a `static LazyLock<T>` uses the default initializer
   type `fn() -> T`, so the autofixable target is a free-standing item.
   For `OnceCell` (which is `!Sync` and so cannot sit in a plain
   `static`): a `thread_local!` static, whose `LazyCell<T>` form is
   equally valid there. A per-instance `Once*` *field* is almost never
   replaceable — see the next section — so struct fields and locals are
   out of scope, which also keeps the analysis tractable.
2. **Every read is `get_or_init`.** Not `get`, not `get_mut`, not
   `set`, not `take`, not `wait`, not `into_inner`, and not
   `get_or_try_init` (see below). A single accessor wrapping one
   `get_or_init` is the canonical shape, but multiple `get_or_init` call
   sites are fine *if* they pass the same initializer.
3. **The initializer is non-capturing.** It must be coercible to
   `fn() -> T`: a closure that captures no local variable and no
   `self`. Referencing other `static` / `const` / `fn` items is not a
   capture and is allowed (those are reachable from the `Lazy*`
   initializer too). A closure that captures a runtime value cannot be
   stored in a `static LazyLock<T>` / `thread_local!` `LazyCell<T>` — its
   type is unnameable there — so it is *not* replaceable.
4. **All `get_or_init` initializers agree.** If two call sites pass
   different closures, there is no single initializer to hoist onto the
   `Lazy*`, so the cell stays a `Once*`.

## When `Lazy*` cannot replace `Once*`

These are the cases `pnpm/pnpm#12477` deliberately left as `OnceLock`;
the same reasoning applies tier-for-tier to `OnceCell`. The lint must
stay silent on every one of them:

- **The value is `set` from outside.** A global configured once at
  startup (`CONFIG.set(parse_args())`), a reporter handle installed by
  the runtime, a test fake injected via `set` — the value is *supplied*
  imperatively, not *computed* by a fixed initializer. `Lazy*` has no
  `set`; its value comes only from the initializer it was constructed
  with. Any `set` / `take` use disqualifies the cell outright.
- **The initializer depends on runtime data.** A per-instance cache
  hydrated from data observed at first access
  (`slot.get_or_init(|| hydrate(observed_version))`), a memo whose
  closure captures `&self` or a function parameter — the closure
  captures a local, so it cannot become a `static`/`thread_local!`
  `Lazy*` initializer. The first caller's data is what `Once*` records;
  `Lazy*` has no first caller to read data from.
- **The init is fallible and handled lazily.** `get_or_try_init`
  returns `Result` so the caller can react to a failed initialisation.
  `Lazy*` has no `try` form — a panicking initializer is its only
  failure mode — so a `get_or_try_init` site is not replaceable.
- **The code inspects initialisation state.** A `get()` that returns
  `Option` to branch on "initialised yet?", or a `set()` whose `Result`
  detects a race, observes a state `Lazy*` does not expose (it is always
  initialised-on-access). Replacing it would drop a meaningful
  distinction.

In short: `Lazy*` fits exactly when the cell is *write-once by a fixed,
self-contained computation, read-only thereafter*. The moment the value
is injected from outside, or the initializer needs data only available
at the call site, or the caller cares about the init result or the
not-yet-init state, it must remain a `Once*`.

## The degenerate case: a plain `static`

A narrower slice of replaceable cells could skip `Lazy*` entirely: if
the initializer is itself a **const expression** (a literal, a `const
fn` call, no heap allocation), then `static FOO: T = <const>;` (or a
`thread_local!` of the same) is simpler still, with no lazy machinery at
all. This is the "could have just been a `static`" case, and it is rare
in practice — most `Once*` initializers allocate (`Vec`, `HashMap`,
`String`, a `Regex`, a thread pool), which is exactly why they were
deferred to runtime in the first place.

Because it is rare and the const-evaluability check is a separate
analysis (a *semantic* property — is every transitively-called function
`const`? — not the *structural* check this rule performs), this rule
does **not** try to detect it. If it proves worth catching, it belongs
in its own sibling rule (a `const`-promotion lint) rather than as a
second diagnostic branch here; bundling two distinct triggers and two
distinct fixes under one banner is the split this catalogue avoids (see
`CLAUDE.md`, "One rule per file"). The note is recorded here so the next
reader knows the omission is deliberate.

## Configuration

```toml
# dylint.toml
#
# Active by default. The rule has a single direction (prefer `Lazy*`
# for the eligible shape), so there is no `style` knob, and it covers
# both std pairs, so there is no per-tier toggle.
[perfectionist::manual_lazy_init]
```

The rule ships no configuration. The one project-level decision it
touches — whether the crate's MSRV permits `std::sync::LazyLock` /
`std::cell::LazyCell` (both Rust 1.80+) — is handled by the activation
mechanism, not a knob: a crate that must support older toolchains
disables the rule via `[perfectionist].disable` rather than configuring
a threshold. Adding an MSRV field here would duplicate machinery the
consumer already has.

## What to lint

`LateLintPass`. Type resolution is required (to confirm the cell's type
and the methods called on it), so this is a late pass, not a token
scan.

1. **Find candidate cells.** Two discovery loci, one per tier:
   - **`OnceLock`:** walk `ItemKind::Static` (and `ItemKind::Const`)
     whose type resolves to `std::sync::OnceLock<T>`. Access is
     `FOO.get_or_init(...)`.
   - **`OnceCell`:** walk `thread_local!`-defined statics whose value
     type resolves to `std::cell::OnceCell<T>` (in the expansion, a
     `LocalKey<OnceCell<T>>`). Access is
     `FOO.with(|c| c.get_or_init(...))`.

   Record each candidate's `DefId` and its `T`.
2. **Collect every use of the cell.** Walk the crate's HIR for paths
   that resolve to that `DefId`. For a named item this is a whole-crate
   `DefId`-equality search — tractable because the item is named, not
   aliased. For each use, classify the `get_or_init` receiver's method
   call (for the `OnceCell` case, the receiver inside the `.with(|c| …)`
   closure):
   - `get_or_init` → record the initializer closure.
   - anything else (`get`, `get_mut`, `set`, `take`, `wait`,
     `into_inner`, `get_or_try_init`) → mark the cell ineligible and
     stop.
3. **Check the initializers.** The cell is replaceable iff:
   - there is at least one `get_or_init` and no disqualifying use;
   - every recorded initializer is **non-capturing**
     (`cx.typeck_results()` / closure upvar list is empty — referencing
     other items is fine, capturing a local or `self` is not); and
   - all initializers are **structurally equal** — identical resolved
     HIR modulo spans, so one closure can be hoisted (see
     "[Equality of initializers](#equality-of-initializers-cost-and-the-scope-boundary)"
     for what "equal" means and why it stops there). The common case is
     a single `get_or_init`, for which this is trivially satisfied.
4. **Emit** on the declaration span, with the rewrite below. The
   diagnostic message names the concrete pair (`OnceLock` → `LazyLock`
   or `OnceCell` → `LazyCell`); the lint *identity* is the same either
   way.

Guard against proc-macro-synthesised nodes per
[`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
("Suppressing proc-macro-synthesised violations"): the diagnostic span
here is the whole declaration, which is wider than an identifier, so the
built-in `report_in_external_macro: false` filter likely suffices — but
a cell generated by a `lazy_static!`-style macro should not be
rewritten, so confirm with a `ui/manual_lazy_init_proc_macro.rs` fixture
and add `crate::common::hir_in_external_macro` if it fires. (The
`thread_local!` locus is itself a macro expansion; candidate-cell
discovery must accept the std `thread_local!` expansion shape while
rejecting third-party lazy-init macros — another reason the guard is
load-bearing for the cell case.)

## Implementation notes

- **Autofix (declaration).** Rewrite
  `static FOO: OnceLock<T> = OnceLock::new();` to
  `static FOO: LazyLock<T> = LazyLock::new(<initializer>);` (and the
  `thread_local!` `OnceCell`/`LazyCell` analogue), splicing in the
  closure pulled from the (single) `get_or_init` site. Update the
  `use std::sync::OnceLock;` / `use std::cell::OnceCell;` import to the
  `Lazy*` form only when the `Once*` type has no other use in the file
  (otherwise add a `Lazy*` import and leave the `Once*` one).
- **Autofix (call sites).** Replace each `FOO.get_or_init(<init>)` with
  `&*FOO` (the `Lazy*: Deref` gives `&T`, the same type `get_or_init`
  returned). An accessor `fn foo() -> &'static T { FOO.get_or_init(...) }`
  collapses to `&FOO`; in the `thread_local!` case the body of
  `with(|c| c.get_or_init(...))` collapses to `with(|c| ...)` using
  `&**c`.
- **Applicability.** `MachineApplicable` only for the canonical
  single-`get_or_init`-via-one-accessor shape, where the rewrite is
  purely mechanical. Downgrade to `MaybeIncorrect` when there are
  multiple call sites (the closure is hoisted once and the others
  deduplicated — correct, but worth a human glance) or when the
  initializer references items that the rewrite's import changes might
  shadow.
- **`const_new`.** `OnceLock::const_new()` is the same eligible shape as
  `OnceLock::new()`; treat both as candidate constructors.

### Difficulty

**Hard.** Unlike most rules in this catalogue, the trigger is not local
to one expression: deciding that a cell is replaceable requires finding
*every* use of it across the crate and proving the negative — that none
of them is a `set`, a `get`, or a capturing / disagreeing initializer.
For a named item the use-set is a `DefId`-equality walk (no alias
analysis), which keeps it feasible, but the "all uses agree and none
disqualify" check is what separates a correct implementation from one
that false-positives on the very cases `pnpm/pnpm#12477` left alone. The
two-tier coverage adds a second discovery locus (the `thread_local!`
`OnceCell` shape) but no new analysis: the use-check core is shared.

A conservative starting implementation:

- Ship the `OnceLock` tier first; the `static` locus is the simplest and
  the one `pnpm/pnpm#12477` exercised. Add the `thread_local!`
  `OnceCell` locus once the `with(|c| c.get_or_init(...))` access shape
  is matched.
- Fire only when the cell has **exactly one** `get_or_init` use and that
  use is its sole reference. This is the single-accessor idiom, the one
  the PR converted most often, and it sidesteps the initializers-agree
  check entirely.
- Defer multi-call-site cells and `const`-promotion to later passes.

### Equality of initializers: cost and the scope boundary

Multiple `get_or_init` sites are eligible when their initializers all
agree (see "What makes a `Once*` replaceable", point 4). Supporting that
does **not** raise the rule's complexity class, *and the reason it
doesn't is itself a design constraint that must be held*. Let `N` be the
crate's HIR size and `E` its expression count (`E ≤ N`).

- **Collecting uses is O(E), and is already paid.** Even the
  single-`get_or_init` MVP must visit every reference to the cell to
  prove no *other* use (`set`, `get`, …) disqualifies it. Additional
  call sites add no new order of work here.
- **Comparing initializers is O(N) total.** Equality is transitive, so
  the `k` closures of one cell are compared **first-against-rest**
  (`c₂…c_k` vs `c₁`), never all-pairs. Each comparison is a lock-step
  structural walk that short-circuits on the first difference, bounded
  by the smaller tree. Every initializer node belongs to exactly one
  cell's use-set, so the sum over all cells is O(N). (Equivalently:
  structural-hash each closure in one O(size) fold and compare the `k`
  hashes in O(k).) **Net: O(N) time, O(E) memory — the same order as
  the base rule.**

That linear bound holds **only** because of two restrictions. They are
the scope boundary: the rule does not cross them, and a request to cross
either one is a request to change the complexity class, not a tweak.
Recorded here so the answer is standing the next time a human or an AI
review proposes "couldn't it also…":

1. **Structural equality, never semantic equivalence.** Two closures are
   "the same" iff their *resolved* HIR is identical modulo spans. HIR
   already carries no whitespace, indentation, or non-doc comments, so
   the "identical except formatting" cases this is meant to catch fall
   out for free. Comparison is over resolved references (`Res` /
   `DefId`), not written path segments, so `|| Registry::load()`
   resolving to two *different* `Registry` types at two sites is
   correctly treated as **unequal**. The rule does **not** ask whether
   two differently-written closures *compute the same value*: general
   program equivalence is undecidable (it reduces to the halting
   problem), and every bounded approximation worth having drags a solver
   or an interprocedural pass into a lint that must stay linear.
   `|| foo()` and `|| { let x = foo(); x }` are deliberately **not**
   merged.
2. **All sites must agree; no partitioning.** Eligibility is
   all-or-nothing — if the `k` initializers are not unanimous, the cell
   is skipped. The rule does **not** find the largest agreeing subset,
   cluster closures into equivalence classes, or suggest splitting one
   `Once*` into several `Lazy*`s. Doing so reintroduces the O(k²)
   all-pairs comparison that first-against-rest exists to avoid, for a
   payoff — a cell deliberately driven by genuinely different
   initializers — that almost never reflects real code.

Two adjacent extensions are out of scope for the same cost reason, noted
so they need not be re-litigated: **interprocedural reasoning** (closures
calling different-but-equivalent helpers — requires inlining) and
**capturing-closure unification** (closures capturing different locals —
already disqualified outright, since a capturing closure cannot be a
`static`/`thread_local!` `Lazy*` initializer at all). Each turns a
linear structural check into a whole-program analysis; neither is
admitted.

**Where this boundary must live in the implementation.** The cost
analysis and the two restrictions above are not merely planning
rationale — they are a standing answer to a recurring review request, so
they belong **in the lint's own rustdoc** (the doc comment on the
`declare_tool_lint!` block, and the rule module's docs), not only here.
This planning file is deleted once the rule ships (per
[`CLAUDE.md`](../CLAUDE.md), "When the implementation is complete"), so
leaving the boundary only in `planned-rules/` would discard it exactly
when it starts mattering. Carry it across verbatim in intent: a future
contributor or AI review proposing "couldn't the equality check also
handle semantically-equivalent closures / partial agreement / helper
inlining?" should find the O(N) bound and its two load-bearing
restrictions next to the code they are about to edit. The implementer's
task is to write that section into the rustdoc; this file's task is only
to require it.

## Default state

Active by default. The eligible shape is unambiguous and the preference
has a single direction, so there is no neutral baseline to omit. The one
caveat is MSRV: `std::sync::LazyLock` and `std::cell::LazyCell` require
Rust 1.80, so a crate pinned below that disables the rule via
`[perfectionist].disable`.

## Interaction with clippy and sibling lints

- **Clippy has no equivalent.** There is no `clippy::manual_lazy_init`;
  the name follows Clippy's `manual_*` idiom (`manual_map`,
  `manual_retain`, …) — "you have manually implemented a `Lazy*`" —
  without mirroring a specific Clippy lint, so it is a fresh
  anti-pattern name, not a borrowed one. It is tier-agnostic by design
  (covers both `OnceLock`→`LazyLock` and `OnceCell`→`LazyCell`); see
  "Why one rule, not two" above.
- **The `once_cell` crate.** `once_cell::sync::OnceCell` /
  `once_cell::unsync::OnceCell` predate the std types and admit a
  similar migration (to `once_cell::sync::Lazy` / `unsync::Lazy`, or,
  post-MSRV-1.80, to std `LazyLock` / `LazyCell`). It is out of scope
  here: the idiomatic fix for a project on recent Rust is to migrate
  `once_cell` to std first, and detecting *that* is a distinct rule with
  a different remedy. The same applies to the `lazy_static!` macro.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
