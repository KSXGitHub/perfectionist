# `in_place_dedup_after_collect`

**Source:** [`KSXGitHub/perfectionist#308`](https://github.com/KSXGitHub/perfectionist/issues/308),
which proposes preferring the
[`into-sorted`](https://crates.io/crates/into-sorted) and
[`into-deduped`](https://crates.io/crates/into-deduped) crates — both by
this project's author — over the in-place `Vec::sort*` / `Vec::dedup*`
mutation that forces a binding to be `mut`. This rule covers the deduping
half; its sibling [`in-place-sort-after-collect`](./in-place-sort-after-collect.md)
covers the sorting half. The two are deliberately parallel.

## Statement

An iterator is collected into a `Vec`, the binding is marked `mut`, and
the *only* reason for the `mut` is an immediately-following in-place
dedup (`Vec::dedup`, `Vec::dedup_by`, or `Vec::dedup_by_key`). The `mut`
would not have been necessary otherwise. The owning dedup methods from
[`into-deduped`](https://crates.io/crates/into-deduped) take the `Vec` by
value and return it deduplicated, so the whole thing stays one expression
and the `mut` binding disappears:

```rust
// Avoid: `mut` exists only so `dedup` can run in place.
let mut ids: Vec<Id> = sorted_rows.iter().map(Row::id).collect();
ids.dedup();
ids

// Prefer: the owning dedup keeps the value in the method chain.
use into_deduped::IntoDeduped;

let ids = sorted_rows
    .iter()
    .map(Row::id)
    .collect::<Vec<_>>()
    .into_deduped();
```

The in-place `Vec` method and the owning `into-deduped` method line up
one-for-one, so the rewrite is purely mechanical:

| In-place (`Vec`, needs `&mut`) | Owning ([`into-deduped`](https://crates.io/crates/into-deduped)) |
|--------------------------------|------------------------------------------------------------------|
| `dedup()`                      | `into_deduped()`                                                 |
| `dedup_by(same)`               | `into_deduped_by(same)`                                          |
| `dedup_by_key(key)`            | `into_deduped_by_key(key)`                                       |

Each method consumes the `Vec` and returns it deduplicated, so the value
never leaves the chain. Like `Vec::dedup*`, the `into_deduped*` methods
remove only **consecutive** duplicates — the `Vec` must already be sorted
to drop *all* duplicates (see [Out of scope](#out-of-scope) on
`itertools::unique`).

## Why restrict this?

This is a stylistic preference, not a correctness issue. The `mut`
binding followed by an in-place `dedup` compiles and produces exactly the
right value. The project prefers the owning form because:

- **The `mut` disappears.** A `mut` binding signals "this value changes
  after creation"; here it does not, beyond the one dedup the rule
  already accounts for. Dropping the `mut` removes a false signal.
- **The value stays in the expression.** `collect().into_deduped()` reads
  as a single pipeline; the three-statement `let mut`/`dedup`/use form
  interrupts it with a name that exists only to host the mutation.
- **No window holding a not-yet-deduped value.** Between the `collect`
  and the `dedup` the binding holds a `Vec` that still has duplicates; a
  later edit reading it there silently gets the wrong contents. The
  owning chain has no such window.

## What to lint

`LateLintPass`. Type resolution is required to confirm the receiver is a
`Vec<T>` and the method is the in-place `dedup*` (not a same-named method
on another type), so this is a late pass, not a token scan.

Fire when **all** of the following hold for a local `let mut` binding:

1. **The initializer is a `collect()` into `Vec`.** The binding's value
   comes from an `Iterator::collect` whose resolved target type is
   `Vec<T>`. The broader "any owned `Vec` expression" case is a
   deliberate out-of-scope extension — see [Out of scope](#out-of-scope).
2. **The binding is `mut`.** A non-`mut` binding cannot host the in-place
   `dedup`, so there is nothing to simplify.
3. **The next use is an in-place `dedup*`.** The first statement that
   mentions the binding is `binding.dedup*(…)` — one of the three methods
   in the table — as a statement-position method call whose return value
   (`()`) is discarded.
4. **The `mut` is needed for nothing else.** After the `dedup`, the
   binding is never mutated again: no further `&mut` borrow, no
   reassignment, no other mutating method, no `&mut`-taking call. (Reads,
   shared borrows, and a final move/return are all fine.) This is the
   load-bearing check — it is what proves the `mut` is *only* there for
   the dedup.

Emit on the `dedup*` call (with the binding's `mut` highlighted as a
secondary span) and offer the autofix below.

### Coordinating with the sort rule

`collect` → `sort` → `dedup` is the canonical sequence. Neither this rule
nor [`in-place-sort-after-collect`](./in-place-sort-after-collect.md)
fires in isolation while the *other's* mutation remains (each rule's
check 4 is failed by the other operation). The two coordinate so a run of
*only* `sort*` / `dedup*` mutations collapses to one
`collect().into_sorted().into_deduped()` chain; see
[Interaction with sibling rules](#interaction-with-sibling-rules).

## Examples

### Dedup-only, after an external sort

**Avoid:**

```rust
fn unique_in_order(sorted: &[Id]) -> Vec<Id> {
    let mut ids: Vec<Id> = sorted.iter().copied().collect();
    ids.dedup();
    ids
}
```

**Prefer:**

```rust
fn unique_in_order(sorted: &[Id]) -> Vec<Id> {
    sorted.iter().copied().collect::<Vec<_>>().into_deduped()
}
```

### Keyed dedup

**Avoid:**

```rust
let mut events: Vec<Event> = stream.collect();
events.dedup_by_key(|e| e.id);
emit(events);
```

**Prefer:**

```rust
let events = stream.collect::<Vec<_>>().into_deduped_by_key(|e| e.id);
emit(events);
```

### Not flagged — the `mut` is needed for more

```rust
// A later in-place mutation genuinely needs `mut`; no owning rewrite applies.
let mut ids: Vec<Id> = rows.iter().map(Row::id).collect();
ids.dedup();
ids.retain(Id::is_live);
```

## Configuration

```toml
# dylint.toml
#
# Active by default. The rule has a single direction (prefer the owning
# `into_deduped*` method) and no per-method toggle.
[perfectionist::in_place_dedup_after_collect]
```

The rule ships no configuration. Whether the consumer's crate depends on
`into-deduped` is handled by the activation mechanism, not a knob: a
crate that does not take the dependency disables the rule via
`[perfectionist].disable`. The autofix assumes the dependency is present
(or addable) — see Implementation notes.

## Out of scope

- **Owned `Vec`s from a source other than `collect`.** `vec![…].dedup()`
  or a `Vec` returned by a helper share the shape and the owning rewrite,
  but the issue scopes this rule to the collect-into-`Vec` case, and the
  name claims no more.
- **Non-consecutive deduplication.** `Vec::dedup*` and `into_deduped*`
  drop only *consecutive* duplicates; the "remove every duplicate
  regardless of position" operation is `itertools::Itertools::unique` /
  `unique_by`, which is backed by a hash set and has **no** `into-deduped`
  equivalent. This rule must not suggest `into_deduped` for a `unique`
  call — the semantics differ — and
  [`itertools-sort-dedup-collect`](./itertools-sort-dedup-collect.md)
  likewise excludes `unique*` for the same reason.
- **In-place dedups whose result feeds a `&mut` consumer.** Covered by
  check 4, which excludes any binding still needing `&mut` afterward.

## Implementation notes

- **Trigger discovery.** Walk `StmtKind::Let` bindings with
  `BindingMode::MUT` whose initializer resolves through
  `cx.typeck_results()` to a `Vec<T>` from `Iterator::collect`; scan the
  enclosing block's following statements for the first mention of the
  binding and match an in-place `dedup*` method-call whose `DefId` is the
  `Vec` inherent method.
- **The "only mutated by the dedup" check.** Identical in shape to the
  sibling rule's check 4: capture the binding's `HirId`, classify every
  subsequent reference, and require each post-dedup use to be a shared
  read or the terminal move. Be conservative — any use the walk cannot
  prove is a shared read disqualifies the binding.
- **Autofix.** Rewrite the
  `let mut x: Vec<T> = <iter>.collect(); x.dedup*(args);` pair into
  `let x = <iter>.collect::<Vec<_>>().into_deduped*(args);` (dropping the
  `mut`, splicing the same `args`), adding `use into_deduped::IntoDeduped;`
  if absent. `MachineApplicable` only when the binding's sole post-dedup
  uses are reads/move *and* the crate already depends on `into-deduped`;
  downgrade to `MaybeIncorrect` when the dependency cannot be confirmed.
  A late pass cannot edit `Cargo.toml`, so the added dependency is a help
  note, never an applied edit.
- **Proc-macro suppression.** The primary span is the `dedup*` method
  call — wider than a bare identifier — so by the "vulnerable exactly
  when the diagnostic span is narrower than the offending node" test in
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

**Medium.** The same shape as the sort sibling: a local block walk plus a
type check to find the trigger, with the correctness risk concentrated in
check 4 (proving the `mut` is used for nothing but the dedup). The
method-mapping table is shorter (three methods, not seven), so this is
marginally the simpler of the pair; the use-analysis core is shared.

## Default state

Active by default. The collect-then-in-place-dedup shape is a broad,
project-agnostic readability point with a single-direction preference.
The dependency caveat is handled by `[perfectionist].disable`, not a
config knob.

## Interaction with sibling rules

- [`in-place-sort-after-collect`](./in-place-sort-after-collect.md) — the
  sorting half of the same proposal. On a `collect` → `sort` → `dedup`
  run, neither rule fires in isolation; the two coordinate to collapse a
  run of only `sort*` / `dedup*` mutations into a single
  `collect().into_sorted().into_deduped()` chain, preserving the source
  order of the two operations.
- [`itertools-sort-dedup-collect`](./itertools-sort-dedup-collect.md) —
  addresses the itertools spelling (`sorted().dedup().collect()`) of the
  same end state and rewrites *toward* the `collect().into_deduped()`
  form this rule targets, so the two never disagree about the
  destination. That rule's exclusion of `itertools::unique*` mirrors this
  rule's: non-consecutive deduplication has no `into-deduped` equivalent.
- **Clippy.** There is no `clippy::manual_into_deduped` or equivalent;
  this is a fresh anti-pattern name, not a borrowed one.
