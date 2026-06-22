# `in_place_sort_after_collect`

**Source:** [`KSXGitHub/perfectionist#308`](https://github.com/KSXGitHub/perfectionist/issues/308),
which proposes preferring the
[`into-sorted`](https://crates.io/crates/into-sorted) and
[`into-deduped`](https://crates.io/crates/into-deduped) crates — both by
this project's author — over the in-place `Vec::sort*` / `Vec::dedup*`
mutation after a collect. This rule covers the sorting half; its sibling
[`in-place-dedup-after-collect`](./in-place-dedup-after-collect.md)
covers the deduping half. The two are deliberately parallel and
**cascade** (see [Interaction](#interaction-with-sibling-rules)).

## Statement

An iterator is collected into a `Vec` and the **very next statement**
sorts it in place — `Vec::sort`, `sort_by`, `sort_by_key`,
`sort_by_cached_key`, or a `sort_unstable*` counterpart. Because the
in-place sort takes `&mut self`, the binding has to be `mut`. The owning
sort methods from [`into-sorted`](https://crates.io/crates/into-sorted)
take the `Vec` by value and return it sorted, so the sort folds back into
the collect chain:

```rust
// Avoid: a separate `mut` binding, sorted in place on the next line.
let mut names: Vec<Name> = people.iter().map(Person::name).collect();
names.sort();
names

// Prefer: the sort folds into the chain.
use into_sorted::IntoSorted;

let names = people.iter().map(Person::name).collect::<Vec<_>>().into_sorted();
```

The in-place `Vec` method and the owning `into-sorted` method line up
one-for-one, so the rewrite is purely mechanical:

| In-place (`Vec`, needs `&mut`) | Owning ([`into-sorted`](https://crates.io/crates/into-sorted)) |
|--------------------------------|----------------------------------------------------------------|
| `sort()`                       | `into_sorted()`                                                |
| `sort_by(f)`                   | `into_sorted_by(f)`                                            |
| `sort_by_key(k)`               | `into_sorted_by_key(k)`                                        |
| `sort_by_cached_key(k)`        | `into_sorted_by_cached_key(k)`                                 |
| `sort_unstable()`              | `into_sorted_unstable()`                                       |
| `sort_unstable_by(f)`          | `into_sorted_unstable_by(f)`                                   |
| `sort_unstable_by_key(k)`      | `into_sorted_unstable_by_key(k)`                               |

Each method consumes the `Vec` and returns it sorted. (The stable
`into_sorted*` methods need the crate's `alloc` feature; the
`into_sorted_unstable*` methods do not. For a `Vec` both are available.)

### The `mut` is not the trigger — `unused_mut` finishes the job

The rule does **not** try to prove the `mut` is unnecessary. That proof
is both costly (a borrow walk over every later use of the binding) and
beside the point, because two facts make it redundant:

- A statement-position `vec.sort()` only type-checks when `vec` is
  already a mutable place, so a `mut` binding is *implied* by the
  trigger — the rule never has to look for it.
- Folding the sort into the initializer is **value-identical**: the
  binding holds the same sorted `Vec` from that point on, so the rewrite
  is correct no matter how `vec` is used afterward.

After the fold, whether the `mut` is still needed is exactly the question
rustc's built-in `unused_mut` already answers. If nothing else mutates
the binding, `unused_mut` fires and offers to drop the `mut`; if
something does (a later `push`), the `mut` stays and is correct. So this
rule keeps the `mut` as-written in its own suggestion and lets
`unused_mut` take it from there. A binding that is sorted and *then*
`push`ed is therefore still flagged — the sort folds in, while the `mut`
and the `push` remain (and `unused_mut` correctly stays quiet).

## Why restrict this?

This is a stylistic preference, not a correctness issue. The collect
followed by an in-place `sort` compiles and produces exactly the right
value. The project prefers the owning form because:

- **The sort stays in the expression.** `collect().into_sorted()` reads
  as one pipeline; the `let mut` / `sort` / use form splits it across a
  name whose only job is to host the mutation.
- **The `mut` usually disappears.** Once the in-place sort is folded out,
  `unused_mut` clears the now-redundant `mut`, so the collect-then-sort
  idiom stops minting `mut` bindings that are never mutated again.
- **No window holding an unsorted value.** Between the `collect` and the
  `sort` the binding holds an *un*-sorted `Vec`; a later edit that reads
  it there is silently wrong. The chain has no such window — and the
  adjacency requirement below guarantees no such read exists today, while
  the chain form keeps it that way under future edits.

## What to lint

`LateLintPass`. Type resolution is required to confirm the receiver is a
`Vec<T>` and the method is the inherent in-place `sort*` (not a
same-named method on another type), so this is a late pass.

Fire when **both** hold:

1. **A `let` binding initialized by a collect-rooted `Vec` chain.** The
   initializer resolves to `Vec<T>` and its method-chain root is an
   `Iterator::collect`. Any intervening calls between that `collect` and
   the binding must themselves be owning `into_sorted*` / `into_deduped*`
   calls — so a chain produced by this rule or its dedup sibling is
   itself an acceptable initializer. This is what lets the two
   [cascade](#the-combined-sort--dedup-sequence).
2. **The immediately following statement sorts it in place.** The *next*
   statement after the `let` is `binding.sort*(args);` in statement
   position with its `()` result discarded, and there is nothing between
   the two statements. This strict adjacency is the simplification that
   replaces dataflow analysis: with no statement in between, nothing can
   observe the intermediate (unsorted) value, so folding the sort into
   the initializer cannot change behaviour.

Emit on the `sort*` call; the autofix folds it into the chain.

### The combined `sort` + `dedup` sequence

`collect` → `sort` → `dedup` is the canonical "sort, then drop the
now-adjacent duplicates" sequence — and the case a per-call,
`mut`-necessity-based design gets wrong (each rule skips it because the
binding is still mutated by the *other* operation). Here it falls out of
the cascade with no special handling. On the original three statements
**only this rule fires**: its trigger is *a `let` binding immediately
followed by a sort*, while the dedup sibling's trigger — *a `let` binding
immediately followed by a dedup* — does not match, because the statement
after the `let` is the sort, not the dedup. After the sort folds:

```rust
let mut v = iter.collect::<Vec<_>>().into_sorted();
v.dedup();
```

the initializer is now a collect-rooted chain and the next statement is
the dedup, so the dedup sibling matches and folds in turn:

```rust
let v = iter.collect::<Vec<_>>().into_sorted().into_deduped();
```

(then `unused_mut` drops the `mut`). Under `cargo dylint --fix` this
resolves over successive iterations; under a plain run the author sees
the sort warning, applies it, then sees the dedup warning. Source order
is preserved because each rule appends its method only when its operation
is the statement immediately following the binding.

## Examples

### Sort-only, returned

**Avoid:**

```rust
fn sorted_names(people: &[Person]) -> Vec<Name> {
    let mut names: Vec<Name> = people.iter().map(Person::name).collect();
    names.sort();
    names
}
```

**Prefer:**

```rust
fn sorted_names(people: &[Person]) -> Vec<Name> {
    people.iter().map(Person::name).collect::<Vec<_>>().into_sorted()
}
```

### Sorted, then pushed — still flagged

**Avoid:**

```rust
let mut names: Vec<Name> = people.iter().map(Person::name).collect();
names.sort();
names.push(Name::sentinel());
```

**Prefer:** the sort folds in; the `mut` and the `push` stay, so
`unused_mut` does *not* fire here —

```rust
let mut names = people.iter().map(Person::name).collect::<Vec<_>>().into_sorted();
names.push(Name::sentinel());
```

### Not flagged — a statement intervenes

```rust
// Something runs between the collect and the sort, so adjacency fails and
// the rule stays silent (conservative — see "What to lint").
let mut names: Vec<Name> = people.iter().map(Person::name).collect();
log::debug!("collected {} names", names.len());
names.sort();
```

### Not flagged — not a collect

```rust
// The chain root is `vec!`, not a collect; out of scope (see "Out of scope").
let mut xs = vec![3, 1, 2];
xs.sort();
```

## Configuration

```toml
# dylint.toml
#
# Active by default. The rule has a single direction (prefer the owning
# `into_sorted*` method) and no per-method toggle.
[perfectionist::in_place_sort_after_collect]
```

The rule ships no configuration. Whether the consumer's crate depends on
`into-sorted` is handled by the activation mechanism, not a knob: a crate
that does not (and will not) take the dependency disables the rule via
`[perfectionist].disable`. The autofix assumes the dependency is present
or addable — see Implementation notes.

## Out of scope

- **Owned `Vec`s from a source other than a collect.** `vec![…].sort()`
  or a `Vec` returned by a helper share the shape and the owning rewrite,
  but the issue scopes this rule to the collect-into-`Vec` case, and the
  name claims no more. The chain root must be an `Iterator::collect`.
- **Slices and arrays.** `into-sorted`'s `IntoSorted` is implemented for
  any `AsMut<[Item]> + Sized` owner, so an in-place-sorted `[T; N]` is
  theoretically in range, but arrays are rarely collected into and the
  payoff is marginal. Left out to keep the trigger tied to the `Vec`
  case.
- **A non-adjacent sort.** If a statement sits between the collect and
  the sort, the rule stays silent rather than reason about whether that
  statement observes the binding. Relaxing this to "no intervening *use*
  of the binding" is a possible later extension, but it reintroduces a
  small use-check the strict-adjacency form avoids.

## Implementation notes

- **Trigger discovery.** Walk `StmtKind::Let` whose initializer resolves
  through `cx.typeck_results()` to `Vec<T>` and whose method-chain root
  is `Iterator::collect` (any intervening calls matched, by trait
  `DefId`, against the `into-sorted` / `into-deduped` owning methods).
  Then confirm the binding's next sibling statement in the block is an
  inherent-`Vec` `sort*` method call on that binding, in statement
  position with its result discarded.
- **No use-analysis.** The rule reads only the two adjacent statements;
  it never enumerates the binding's later uses. Correctness comes from
  adjacency (nothing observes the intermediate value) plus value-identity
  of the fold, not from proving the `mut` is dead — that is `unused_mut`'s
  job, deliberately left to it.
- **Autofix.** Rewrite the `let … = <chain>; binding.sort*(args);` pair
  into `let … = <chain>.into_sorted*(args);`, deleting the sort
  statement and adding `use into_sorted::IntoSorted;` / `IntoSortedUnstable`
  if absent. Keep the binding's `mut` exactly as written — `unused_mut`
  removes it when it becomes redundant. Ensure the collect carries an
  explicit `::<Vec<_>>()` turbofish when its type was previously fixed
  only by the `let` annotation or by the now-removed `sort` call, so the
  owning method resolves. `MachineApplicable` when the crate already
  depends on `into-sorted`; otherwise `MaybeIncorrect`, since a late pass
  cannot add the dependency to `Cargo.toml`.
- **Proc-macro suppression.** The primary span is the `sort*` call —
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

**Easy–Medium.** Dropping the `mut`-necessity proof removes the hard
part. What remains is a two-adjacent-statement structural match plus one
type check, and an autofix that folds a call and threads a `Vec<_>`
turbofish. The cascade needs no special handling — it is just the same
rule (and its sibling) re-firing on its own output, because the rewritten
initializer is again a collect-rooted chain.

## Default state

Active by default. The collect-then-in-place-sort shape is a broad,
project-agnostic readability point and the preference has a single
direction. The dependency caveat is handled by `[perfectionist].disable`,
not a config knob.

## Interaction with sibling rules

- [`in-place-dedup-after-collect`](./in-place-dedup-after-collect.md) —
  the deduping half. The two **cascade**: each accepts a collect-rooted
  owning chain as its initializer and folds the in-place operation that
  immediately follows it, so `collect` → `sort` → `dedup` collapses to a
  single `collect().into_sorted().into_deduped()` over successive fixes,
  source order preserved. Neither needs to know about the other's
  operation; each just re-fires on the chain the other produced.
- [`itertools-sort-dedup-collect`](./itertools-sort-dedup-collect.md) —
  the itertools spelling (`sorted().dedup().collect()`) of the same end
  state, rewriting *toward* the `collect().into_sorted()` form this rule
  produces, so the two never disagree about the destination.
- **Clippy.** `clippy::needless_collect` flags a *different* anti-pattern
  (collecting only to immediately re-iterate). `unused_mut` is this
  rule's load-bearing partner: it removes the `mut` the fold leaves
  behind, which is why this rule does not attempt that removal itself.
