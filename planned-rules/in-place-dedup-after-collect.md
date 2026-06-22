# `in_place_dedup_after_collect`

**Source:** [`KSXGitHub/perfectionist#308`](https://github.com/KSXGitHub/perfectionist/issues/308),
which proposes preferring the
[`into-sorted`](https://crates.io/crates/into-sorted) and
[`into-deduped`](https://crates.io/crates/into-deduped) crates — both by
this project's author — over the in-place `Vec::sort*` / `Vec::dedup*`
mutation after a collect. This rule covers the deduping half; its sibling
[`in-place-sort-after-collect`](./in-place-sort-after-collect.md) covers
the sorting half. The two are deliberately parallel and **cascade** (see
[Interaction](#interaction-with-sibling-rules)).

## Statement

An iterator is collected into a `Vec` and the **very next statement**
deduplicates it in place — `Vec::dedup`, `Vec::dedup_by`, or
`Vec::dedup_by_key`. Because the in-place dedup takes `&mut self`, the
binding has to be `mut`. The owning dedup methods from
[`into-deduped`](https://crates.io/crates/into-deduped) take the `Vec` by
value and return it deduplicated, so the dedup folds back into the
collect chain:

```rust
// Avoid: a separate `mut` binding, deduped in place on the next line.
let mut ids: Vec<Id> = sorted_rows.iter().map(Row::id).collect();
ids.dedup();
ids

// Prefer: the dedup folds into the chain.
use into_deduped::IntoDeduped;

let ids = sorted_rows.iter().map(Row::id).collect::<Vec<_>>().into_deduped();
```

The in-place `Vec` method and the owning `into-deduped` method line up
one-for-one, so the rewrite is purely mechanical:

| In-place (`Vec`, needs `&mut`) | Owning ([`into-deduped`](https://crates.io/crates/into-deduped)) |
|--------------------------------|------------------------------------------------------------------|
| `dedup()`                      | `into_deduped()`                                                 |
| `dedup_by(same)`               | `into_deduped_by(same)`                                          |
| `dedup_by_key(key)`            | `into_deduped_by_key(key)`                                       |

Each method consumes the `Vec` and returns it deduplicated. Like
`Vec::dedup*`, the `into_deduped*` methods remove only **consecutive**
duplicates — the `Vec` must already be sorted to drop *all* duplicates
(see [Out of scope](#out-of-scope) on `itertools::unique`).

### The `mut` is not the trigger — `unused_mut` finishes the job

The rule does **not** try to prove the `mut` is unnecessary. That proof
is both costly (a borrow walk over every later use of the binding) and
beside the point, because two facts make it redundant:

- A statement-position `vec.dedup()` only type-checks when `vec` is
  already a mutable place, so a `mut` binding is *implied* by the
  trigger — the rule never has to look for it.
- Folding the dedup into the initializer is **value-identical**: the
  binding holds the same deduplicated `Vec` from that point on, so the
  rewrite is correct no matter how `vec` is used afterward.

After the fold, whether the `mut` is still needed is exactly the question
rustc's built-in `unused_mut` already answers. If nothing else mutates
the binding, `unused_mut` fires and offers to drop the `mut`; if
something does (a later `push`), the `mut` stays and is correct. So this
rule keeps the `mut` as-written in its own suggestion and lets
`unused_mut` take it from there. A binding that is deduped and *then*
`push`ed is therefore still flagged — the dedup folds in, while the `mut`
and the `push` remain (and `unused_mut` correctly stays quiet).

## Why restrict this?

This is a stylistic preference, not a correctness issue. The collect
followed by an in-place `dedup` compiles and produces exactly the right
value. The project prefers the owning form because:

- **The dedup stays in the expression.** `collect().into_deduped()` reads
  as one pipeline; the `let mut` / `dedup` / use form splits it across a
  name whose only job is to host the mutation.
- **The `mut` usually disappears.** Once the in-place dedup is folded
  out, `unused_mut` clears the now-redundant `mut`, so the
  collect-then-dedup idiom stops minting `mut` bindings that are never
  mutated again.
- **No window holding a not-yet-deduped value.** Between the `collect`
  and the `dedup` the binding holds a `Vec` that still has duplicates; a
  later edit that reads it there is silently wrong. The chain has no such
  window — and the adjacency requirement below guarantees no such read
  exists today, while the chain form keeps it that way under future
  edits.

## What to lint

`LateLintPass`. Type resolution is required to confirm the receiver is a
`Vec<T>` and the method is the inherent in-place `dedup*` (not a
same-named method on another type), so this is a late pass.

Fire when **both** hold:

1. **A `let` binding initialized by a collect-rooted `Vec` chain.** The
   initializer resolves to `Vec<T>` and its method-chain root is an
   `Iterator::collect`. Any intervening calls between that `collect` and
   the binding must themselves be owning `into_sorted*` / `into_deduped*`
   calls — so a chain produced by this rule or its sort sibling is itself
   an acceptable initializer. This is what lets the two
   [cascade](#the-combined-sort--dedup-sequence).
2. **The immediately following statement dedups it in place.** The *next*
   statement after the `let` is `binding.dedup*(args);` in statement
   position with its `()` result discarded, and there is nothing between
   the two statements. This strict adjacency is the simplification that
   replaces dataflow analysis: with no statement in between, nothing can
   observe the intermediate (not-yet-deduped) value, so folding the dedup
   into the initializer cannot change behaviour.

Emit on the `dedup*` call; the autofix folds it into the chain.

### The combined `sort` + `dedup` sequence

`collect` → `sort` → `dedup` is the canonical "sort, then drop the
now-adjacent duplicates" sequence — and the case a per-call,
`mut`-necessity-based design gets wrong (each rule skips it because the
binding is still mutated by the *other* operation). Here it falls out of
the cascade. On the original three statements **only the sort sibling
fires** (the statement after the `let` is the sort, not the dedup, so
*this* rule does not match yet). Once the sort has folded:

```rust
let mut v = iter.collect::<Vec<_>>().into_sorted();
v.dedup();
```

the initializer is a collect-rooted chain and the next statement is the
dedup, so this rule now matches and folds in turn:

```rust
let v = iter.collect::<Vec<_>>().into_sorted().into_deduped();
```

(then `unused_mut` drops the `mut`). Under `cargo dylint --fix` this
resolves over successive iterations; under a plain run the author sees
the sort warning, applies it, then sees the dedup warning. Source order
is preserved because each rule appends its method only when its operation
is the statement immediately following the binding.

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

### Deduped, then pushed — still flagged

**Avoid:**

```rust
let mut ids: Vec<Id> = rows.iter().map(Row::id).collect();
ids.dedup();
ids.push(Id::sentinel());
```

**Prefer:** the dedup folds in; the `mut` and the `push` stay, so
`unused_mut` does *not* fire here —

```rust
let mut ids = rows.iter().map(Row::id).collect::<Vec<_>>().into_deduped();
ids.push(Id::sentinel());
```

### Not flagged — a statement intervenes

```rust
// Something runs between the collect and the dedup, so adjacency fails
// and the rule stays silent (conservative — see "What to lint").
let mut ids: Vec<Id> = rows.iter().map(Row::id).collect();
log::debug!("collected {} ids", ids.len());
ids.dedup();
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
or addable — see Implementation notes.

## Out of scope

- **Owned `Vec`s from a source other than a collect.** `vec![…].dedup()`
  or a `Vec` returned by a helper share the shape and the owning rewrite,
  but the issue scopes this rule to the collect-into-`Vec` case, and the
  name claims no more. The chain root must be an `Iterator::collect`.
- **Non-consecutive deduplication.** `Vec::dedup*` and `into_deduped*`
  drop only *consecutive* duplicates; the "remove every duplicate
  regardless of position" operation is `itertools::Itertools::unique` /
  `unique_by`, backed by a hash set, with **no** `into-deduped`
  equivalent. This rule must not suggest `into_deduped` for a `unique`
  call — the semantics differ — and
  [`itertools-sort-dedup-collect`](./itertools-sort-dedup-collect.md)
  likewise excludes `unique*`.
- **A non-adjacent dedup.** If a statement sits between the collect and
  the dedup, the rule stays silent rather than reason about whether that
  statement observes the binding. Relaxing this is a possible later
  extension, but it reintroduces a small use-check the strict-adjacency
  form avoids.

## Implementation notes

- **Trigger discovery.** Walk `StmtKind::Let` whose initializer resolves
  through `cx.typeck_results()` to `Vec<T>` and whose method-chain root
  is `Iterator::collect` (any intervening calls matched, by trait
  `DefId`, against the `into-sorted` / `into-deduped` owning methods).
  Then confirm the binding's next sibling statement in the block is an
  inherent-`Vec` `dedup*` method call on that binding, in statement
  position with its result discarded.
- **No use-analysis.** The rule reads only the two adjacent statements;
  it never enumerates the binding's later uses. Correctness comes from
  adjacency (nothing observes the intermediate value) plus value-identity
  of the fold, not from proving the `mut` is dead — that is `unused_mut`'s
  job, deliberately left to it.
- **Autofix.** Rewrite the `let … = <chain>; binding.dedup*(args);` pair
  into `let … = <chain>.into_deduped*(args);`, deleting the dedup
  statement and adding `use into_deduped::IntoDeduped;` if absent. Keep
  the binding's `mut` exactly as written — `unused_mut` removes it when it
  becomes redundant. Ensure the collect carries an explicit `::<Vec<_>>()`
  turbofish when its type was previously fixed only by the `let`
  annotation or by the now-removed `dedup` call, so the owning method
  resolves. `MachineApplicable` when the crate already depends on
  `into-deduped`; otherwise `MaybeIncorrect`, since a late pass cannot add
  the dependency to `Cargo.toml`.
- **Proc-macro suppression.** The primary span is the `dedup*` call —
  wider than a bare identifier — so by the "vulnerable exactly when the
  diagnostic span is narrower than the offending node" test in
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

**Easy–Medium.** The same shape as the sort sibling, with a shorter
method table (three methods, not seven), so marginally the simpler of the
pair. Dropping the `mut`-necessity proof removes the hard part; what
remains is a two-adjacent-statement match, one type check, and a
call-folding autofix. The cascade needs no special handling — it is just
the rule re-firing on the chain its sort sibling produced.

## Default state

Active by default. The collect-then-in-place-dedup shape is a broad,
project-agnostic readability point with a single-direction preference.
The dependency caveat is handled by `[perfectionist].disable`, not a
config knob.

## Interaction with sibling rules

- [`in-place-sort-after-collect`](./in-place-sort-after-collect.md) — the
  sorting half. The two **cascade**: each accepts a collect-rooted owning
  chain as its initializer and folds the in-place operation that
  immediately follows it, so `collect` → `sort` → `dedup` collapses to a
  single `collect().into_sorted().into_deduped()` over successive fixes,
  source order preserved. Neither needs to know about the other's
  operation; each just re-fires on the chain the other produced.
- [`itertools-sort-dedup-collect`](./itertools-sort-dedup-collect.md) —
  the itertools spelling, rewriting *toward* the `collect().into_deduped()`
  form this rule produces. Its exclusion of `itertools::unique*` mirrors
  this rule's: non-consecutive deduplication has no `into-deduped`
  equivalent.
- **Clippy.** There is no `clippy::manual_into_deduped` equivalent;
  `unused_mut` is this rule's load-bearing partner, removing the `mut` the
  fold leaves behind, which is why this rule does not attempt that
  removal itself.
