# `itertools_sort_dedup_collect`

**Source:** [`KSXGitHub/perfectionist#308`](https://github.com/KSXGitHub/perfectionist/issues/308),
which proposes preferring the
[`into-sorted`](https://crates.io/crates/into-sorted) and
[`into-deduped`](https://crates.io/crates/into-deduped) crates — both by
this project's author — over the
[`itertools`](https://crates.io/crates/itertools) `sorted*` / `dedup*`
iterator adaptors when their result is immediately collected into a
`Vec`. This is the codegen half of the proposal; the
`in-place-sort` / `in-place-dedup` pair
covers the `mut`-elision half.

## Statement

`itertools` provides iterator adaptors that sort
(`Itertools::sorted`, `sorted_by`, `sorted_by_key`, `sorted_unstable*`)
and deduplicate (`Itertools::dedup`, `dedup_by`) an iterator. When the
result is immediately `collect()`ed into a `Vec`, the same end value can
be produced by collecting *first* and then applying the owning
[`into-sorted`](https://crates.io/crates/into-sorted) /
[`into-deduped`](https://crates.io/crates/into-deduped) methods:

```rust
// A — itertools adaptors, then collect:
let out: Vec<_> = iter.sorted().dedup().collect();

// B — collect first, then the owning methods (preferred):
use into_sorted::IntoSorted;
use into_deduped::IntoDeduped;

let out = iter.collect::<Vec<_>>().into_sorted().into_deduped();
```

For simple cases A and B compile to identical machine code. But for
non-trivial element types and chains, A and B **differ**: inspected on an
ASM viewer at `opt-level = 3`, B produces tighter code than A (A carries
the overhead of itertools' adaptor plumbing — `sorted` internally
collects into a `Vec`, sorts, and hands back a `vec::IntoIter`, which the
trailing `.collect()` then drains into a second `Vec`; `dedup` layers a
lazy peeking adaptor on top). B and the equivalent manual
collect-sort-dedup produce the same binary as each other. The rule flags
A and suggests B.

The adaptors map one-for-one onto the owning methods:

| itertools adaptor                 | owning replacement                              |
|-----------------------------------|-------------------------------------------------|
| `.sorted()`                       | `.into_sorted()`                                |
| `.sorted_by(f)`                   | `.into_sorted_by(f)`                            |
| `.sorted_by_key(k)`               | `.into_sorted_by_key(k)`                        |
| `.sorted_unstable()`              | `.into_sorted_unstable()`                       |
| `.sorted_unstable_by(f)`          | `.into_sorted_unstable_by(f)`                   |
| `.sorted_unstable_by_key(k)`      | `.into_sorted_unstable_by_key(k)`               |
| `.dedup()`                        | `.into_deduped()`                               |
| `.dedup_by(same)`                 | `.into_deduped_by(same)`                        |

The terminal `.collect::<Vec<_>>()` of chain A becomes the *leading*
`.collect::<Vec<_>>()` of chain B, onto which the owning methods are
appended in source order.

### Why one rule, not two (sort + dedup)

Unlike Problem 1's `mut`-elision pair — where the issue explicitly asks
for "one for the sorting, one for the deduping" — the itertools case is
naturally **one rule**, because the adaptors *compose in a single chain*
that must be rewritten as a unit. On `iter.sorted().dedup().collect()`
the rewrite has to lift the *one* terminal `collect()` to the front and
append both owning methods; two independent rules would each see only
part of the chain and could emit overlapping, conflicting suggestions for
the same `collect()`. One rule that recognises a maximal run of
`sorted*` / `dedup*` adaptors terminated by a `Vec` `collect()` and
rewrites the whole run at once is the coherent shape. The trigger
predicate, the diagnostic, and the autofix are shared; only the
per-adaptor name lookup differs, which is a table, not a second rule.
(Contrast Problem 1, where sort and dedup are *separate statements*
mutating a binding, so splitting them is the natural decomposition.)

If a future need to enable only one direction emerges, the split can be
revisited; the issue frames Problem 2 as a single rule and this file
follows that.

## Why restrict this?

This is a stylistic preference, not a correctness issue — both spellings
compute the same `Vec`. The project prefers B for two reasons:

- **Codegen.** Per the issue, on complex element types and chains at
  `opt-level = 3` the itertools form (A) compiles to measurably worse
  machine code than the collect-first form (B); B matches the binary of
  the hand-written collect-sort-dedup. Avoiding the itertools adaptor
  layers when the result is going into a `Vec` anyway costs nothing and
  can only help the optimiser.
- **Consistency with the `mut`-elision rules.** B is exactly the
  `collect().into_sorted().into_deduped()` shape that
  [`in-place-sort`](./in-place-sort.md) and
  [`in-place-dedup`](./in-place-dedup.md)
  steer the *imperative* spelling toward, so one project-wide form wins.

Because both forms are correct, the rule is a preference, not a
correctness fix — even though the motivation is partly a measured codegen
difference. (When in doubt, the catalogue's convention is to treat a rule
as a preference; a slower-but-correct binary is not "wrong on its own
terms".)

## What to lint

`LateLintPass`. Type resolution is required to confirm the adaptors are
the `itertools::Itertools` methods (not same-named inherent or
third-party methods) and that the terminal `collect` targets `Vec<T>`, so
this is a late pass.

Fire on a method-call chain where **all** hold:

1. **A terminal `collect` into `Vec`.** The outermost call is
   `Iterator::collect` whose resolved result type is `Vec<T>`.
2. **Its receiver is a run of itertools `sorted*` / `dedup*` adaptors.**
   Walking inward from the `collect`, the immediately-preceding calls
   resolve to `itertools::Itertools` methods in the mapped set above. The
   run is *maximal*: include every consecutive such adaptor.
3. **At least one adaptor in the run is `sorted*` or `dedup*`.** A chain
   of only non-mapped itertools adaptors does not fire.

Emit on the chain (anchored at the first mapped adaptor through the
`collect`) and offer the autofix below.

### Excluded itertools adaptors

- **`unique` / `unique_by`.** These remove *non-consecutive* duplicates
  via a hash set; `into_deduped*` (like `Vec::dedup*`) removes only
  *consecutive* duplicates. They are **not** equivalent, so a `unique*`
  call is never rewritten to `into_deduped*` and never extends the run.
  (Same exclusion as
  [`in-place-dedup`](./in-place-dedup.md).)
- **`dedup_with_count` / `dedup_by_with_count`.** These yield
  `(count, elem)` pairs — a different element type — so they have no
  `into_deduped` counterpart.
- **`k_smallest` / `sorted_by_cached_key`-style adaptors not present in
  itertools.** Only the adaptors in the mapping table are rewritten; an
  unmapped adaptor terminates the run (the `collect` of a chain whose
  innermost mapped run is empty does not fire).
- **A non-`Vec` collect target** (`collect::<BTreeSet<_>>()`,
  `collect::<HashSet<_>>()`, …). The owning methods are `Vec`-shaped; a
  different container is out of scope.

## Examples

### Sorted + deduped, collected

**Avoid:**

```rust
let names: Vec<Name> = people.iter().map(Person::name).sorted().dedup().collect();
```

**Prefer:**

```rust
let names = people
    .iter()
    .map(Person::name)
    .collect::<Vec<_>>()
    .into_sorted()
    .into_deduped();
```

### Keyed sort only

**Avoid:**

```rust
let entries: Vec<Entry> = stream.sorted_by_key(|e| e.priority).collect();
```

**Prefer:**

```rust
let entries = stream.collect::<Vec<_>>().into_sorted_by_key(|e| e.priority);
```

### Not flagged — `unique`, not `dedup`

```rust
// `unique` removes non-consecutive duplicates; no `into_deduped` equivalent.
let ids: Vec<Id> = rows.iter().map(Row::id).unique().collect();
```

### Not flagged — not collected into a `Vec`

```rust
// Lazy use of the adaptor, never collected into a Vec.
for x in iter.sorted() {
    consume(x);
}
```

## Configuration

```toml
# dylint.toml
#
# Active by default. Single direction (prefer the collect-first owning
# form); no per-adaptor toggle.
[perfectionist::itertools_sort_dedup_collect]
```

The rule ships no configuration. Whether the consumer depends on
`into-sorted` / `into-deduped` is handled by `[perfectionist].disable`,
not a knob; the autofix assumes the dependencies are present or addable
(see Implementation notes). The rule only fires in crates that already
depend on `itertools` (the adaptors cannot resolve otherwise).

## Implementation notes

- **Trigger discovery.** On `ExprKind::MethodCall` for `collect`, confirm
  the result type is `Vec<T>` via `cx.typeck_results()`, then walk the
  receiver chain inward, matching each `MethodCall` `DefId` against the
  `itertools::Itertools` `sorted*` / `dedup*` set. Resolve via `DefId`,
  not method name, so an inherent or third-party `sorted` on some other
  type does not match.
- **Autofix.** Move the terminal `collect::<Vec<_>>()` to the front of
  the matched run and replace each adaptor with its owning method in
  source order: `base.<adaptors…>.collect()` →
  `base.collect::<Vec<_>>().<into_…>()`. Add `use into_sorted::…;` /
  `use into_deduped::…;` imports as needed. `MachineApplicable` only when
  the crate already depends on the relevant `into-*` crate(s); otherwise
  `MaybeIncorrect`, leaving the author to add the dependency (a late pass
  cannot edit `Cargo.toml`).
- **Turbofish vs annotation.** The original `collect` may have been typed
  by a `let` annotation rather than a turbofish; the rewritten leading
  `collect::<Vec<_>>()` must carry an explicit `Vec<_>` turbofish so the
  owning method resolves, even when the original relied on the
  annotation. Drop the now-redundant `let` annotation or keep it — both
  type-check; prefer the turbofish form for locality.
- **Proc-macro suppression.** The primary span covers the adaptor-run
  through the `collect` — wider than a bare identifier — so by the
  "vulnerable exactly when the diagnostic span is narrower than the
  offending node" test in
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  ("Suppressing proc-macro-synthesised violations"), the built-in
  `report_in_external_macro: false` filter suffices; no
  `hir_in_external_macro` guard or `ui/<rule>_proc_macro.rs` fixture is
  required. Record that reasoning at the span-selection site.
- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

### Difficulty

**Medium.** The chain walk and `DefId` matching are mechanical; the care
goes into (1) bounding the run correctly — stopping at the first unmapped
or excluded adaptor, and not mis-firing on `unique*` — and (2) the
autofix's collect-lifting, which reorders the chain and must thread the
`Vec<_>` turbofish through. Verifying the codegen claim is not the lint's
job; the rule only recognises the syntactic shape the issue identified.

## Default state

Active by default. The shape is project-agnostic and the preference has a
single direction. The dependency caveat (`into-sorted` / `into-deduped`,
and the rule only fires where `itertools` is already a dependency) is
handled by `[perfectionist].disable`, not a config knob.

## Interaction with sibling rules

- [`in-place-sort`](./in-place-sort.md) /
  [`in-place-dedup`](./in-place-dedup.md) —
  the imperative-spelling half of the same proposal. Those rules fold an
  owned-`Vec` binding's immediately-following in-place `sort`/`dedup`
  *toward* the `collect().into_sorted().into_deduped()` form this rule
  produces, so all three converge on one destination. They never overlap
  on a single expression: this rule fires on the itertools-adaptor chain,
  those on an owned-`Vec` binding sorted/deduped in place by
  `Vec::sort*` / `Vec::dedup*` on the next statement.
- **`clippy::needless_collect`** flags collecting an iterator only to
  immediately re-consume it; this rule's *output* deliberately inserts a
  `collect()` (the collect-first form is the point), so it does not feed
  `needless_collect` — the collected `Vec` is the owning methods'
  receiver, not a throwaway re-iterated immediately.
- **itertools' own lint guidance.** itertools does not lint against its
  adaptors; this rule encodes a project-specific codegen preference, not
  a correction of itertools.
