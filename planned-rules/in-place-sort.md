# `in_place_sort`

**Source:** [`KSXGitHub/perfectionist#308`](https://github.com/KSXGitHub/perfectionist/issues/308),
which proposes preferring the
[`into-sorted`](https://crates.io/crates/into-sorted) and
[`into-deduped`](https://crates.io/crates/into-deduped) crates — both by
this project's author — over the in-place `Vec::sort*` / `Vec::dedup*`
mutation of a freshly-bound `Vec`. This rule covers the sorting half; its
sibling [`in-place-dedup`](./in-place-dedup.md) covers the deduping half.
The two are deliberately parallel and **cascade** (see
[Interaction](#interaction-with-sibling-rules)).

## Statement

A `Vec` is bound by value and the **very next statement** sorts it in
place — `Vec::sort`, `sort_by`, `sort_by_key`, `sort_by_cached_key`, or a
`sort_unstable*` counterpart. Because the in-place sort takes `&mut self`,
the binding has to be `mut`. The owning sort methods from
[`into-sorted`](https://crates.io/crates/into-sorted) take the `Vec` by
value and return it sorted, so the sort folds back into the initializer:

```rust
// Avoid: a separate `mut` binding, sorted in place on the next line.
let mut names: Vec<Name> = people.iter().map(Person::name).collect();
names.sort();
names

// Prefer: the sort folds into the initializer.
use into_sorted::IntoSorted;

let names = people.iter().map(Person::name).collect::<Vec<_>>().into_sorted();
```

The binding's initializer can be **any** owned `Vec` expression — a
`collect()`, a `vec![…]` literal, a `Vec`-returning call, a moved
binding. Where the `Vec` comes from does not matter; the fold only needs
the initializer to be a `Vec` value the binding owns, and it is
value-identical in every case:

```rust
// All three fold the same way.
let mut a = load_paths();          a.sort();   // → load_paths().into_sorted()
let mut b = vec![3, 1, 2];         b.sort();   // → vec![3, 1, 2].into_sorted()
let mut c: Vec<_> = xs.collect();  c.sort();   // → xs.collect::<Vec<_>>().into_sorted()
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

### The `mut`'s *necessity* is not the trigger — its *presence* is guaranteed

Two questions about the `mut` are easy to conflate; the rule takes
opposite stances on them.

**Presence — guaranteed, and safe to rely on.** `Vec::sort*` takes
`&mut self`, so the receiver place must be mutable, and for a binding
that *owns* its `Vec` by value the only way to get that is `let mut`.
rustc rejects the alternative outright: `let v = vec![3, 1, 2];
v.sort();` is E0596, "cannot borrow `v` as mutable, as it is not declared
as mutable". Every binding this rule can match is therefore already
`mut`, and the implementation may lean on that — the binding mode is a
sound, cheap pre-filter (skip a non-`mut` binding before paying for type
resolution), and the autofix keeps the `mut` as written.

**Necessity — never analysed.** Whether the `mut` is *still* needed once
the sort folds away is a different question, and proving it costs a
borrow walk over every later use of the binding. The rule does not do
that, and a future implementation must not make it a trigger condition,
because two facts make the proof redundant:

- Folding the sort into the initializer is **value-identical**: the
  binding holds the same sorted `Vec` from that point on, so the rewrite
  is correct no matter how `vec` is used afterward.
- rustc's built-in `unused_mut` already answers the question after the
  fold. If nothing else mutates the binding, it fires and offers to drop
  the `mut`; if something does (a later `push`), the `mut` stays and is
  correct.

So this rule keeps the `mut` as-written in its own suggestion and lets
`unused_mut` take it from there. A binding that is sorted and *then*
`push`ed is therefore still flagged — the sort folds in, while the `mut`
and the `push` remain (and `unused_mut` correctly stays quiet).

> [!IMPORTANT]
> **The presence guarantee is scoped to by-value bindings.** It holds
> *because* the trigger is restricted to a binding that owns its `Vec`
> (see [What to lint](#what-to-lint)). A `&mut Vec<T>` binding needs no
> `mut` of its own — `let v: &mut Vec<i32> = buf; v.sort();` compiles,
> because the mutability rides on the reference rather than the binding.
> Such bindings are out of scope anyway (`into_sorted` consumes by
> value), but the check must read the **binding's** type, not the
> initializer expression's: `let ref mut v = vec![3, 1, 2];` has an
> initializer of type `Vec<i32>` and a binding of type `&mut Vec<i32>`,
> with no `mut` binding mode in sight.

## Why restrict this?

This is a stylistic preference, not a correctness issue. Binding a `Vec`
and sorting it in place compiles and produces exactly the right value.
The project prefers the owning form because:

- **The sort stays in the expression.** `<init>.into_sorted()` reads as
  one pipeline; the `let mut` / `sort` / use form splits it across a name
  whose only job is to host the mutation.
- **The `mut` usually disappears.** Once the in-place sort is folded out,
  `unused_mut` clears the now-redundant `mut`, so the bind-then-sort idiom
  stops minting `mut` bindings that are never mutated again.
- **No window holding an unsorted value.** Between the binding and the
  `sort` the binding holds an *un*-sorted `Vec`; a later edit that reads
  it there is silently wrong. The chain has no such window — and the
  adjacency requirement below guarantees no such read exists today, while
  the chain form keeps it that way under future edits.

## What to lint

`LateLintPass`. Type resolution is required to confirm the receiver is a
`Vec<T>` the binding owns and the method is the inherent in-place `sort*`
(not a same-named method on another type), so this is a late pass.

Fire when **both** hold:

1. **A `let` binding whose initializer is an owned `Vec`.** The
   initializer resolves through `cx.typeck_results()` to `Vec<T>` and the
   binding owns it by value (not `&Vec<T>` / `&mut Vec<T>`). The
   *syntactic* source of the `Vec` is irrelevant — a `collect()`, a
   `vec![…]`, a `Vec`-returning call, or a chain already ending in an
   owning `into_sorted*` / `into_deduped*` method all qualify. (The last
   is what lets the two rules [cascade](#the-combined-sort--dedup-sequence);
   no special-casing is needed, because such a chain is itself just
   another owned-`Vec` initializer.)
2. **The immediately following statement sorts it in place.** The *next*
   statement after the `let` is `binding.sort*(args);` in statement
   position with its `()` result discarded, and there is nothing between
   the two statements. This strict adjacency is the simplification that
   replaces dataflow analysis: with no statement in between, nothing can
   observe the intermediate (unsorted) value, so folding the sort into the
   initializer cannot change behaviour.

Emit on the `sort*` call; the autofix folds it into the initializer.

### The combined `sort` + `dedup` sequence

A bind → `sort` → `dedup` run (canonically `collect` → `sort` → `dedup`,
"sort, then drop the now-adjacent duplicates") is the case a per-call,
`mut`-necessity-based design gets wrong: each rule skips it because the
binding is still mutated by the *other* operation. Here it falls out of
the cascade with no special handling. On the original three statements
**only this rule fires**: its trigger is *a `let`-bound owned `Vec`
immediately followed by a sort*, while the dedup sibling's trigger — *a
binding immediately followed by a dedup* — does not match, because the
statement after the `let` is the sort, not the dedup. After the sort
folds:

```rust
let mut v = iter.collect::<Vec<_>>().into_sorted();
v.dedup();
```

the initializer is still an owned `Vec` (a chain now ending in
`into_sorted()`) and the next statement is the dedup, so the dedup sibling
matches and folds in turn:

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

### A `vec!` literal — also flagged

**Avoid:**

```rust
let mut xs = vec![3, 1, 2];
xs.sort();
```

**Prefer:** the source is a literal, not a collect, but the fold is the
same (and needs no turbofish — `vec![…]` already has a concrete type) —

```rust
let xs = vec![3, 1, 2].into_sorted();
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
// Something runs between the binding and the sort, so adjacency fails and
// the rule stays silent (conservative — see "What to lint").
let mut names: Vec<Name> = people.iter().map(Person::name).collect();
log::debug!("collected {} names", names.len());
names.sort();
```

### Not flagged — the binding does not own the `Vec`

```rust
// `v` is a `&mut Vec<_>`, not an owned `Vec`. `into_sorted` consumes the
// `Vec` by value, which a reference cannot provide, so this is out of
// scope (see "Out of scope").
let v: &mut Vec<u8> = &mut buffer;
v.sort();
```

## Configuration

```toml
# dylint.toml
#
# Active by default. The rule has a single direction (prefer the owning
# `into_sorted*` method) and no per-method toggle.
["perfectionist::in_place_sort"]
```

The rule ships no configuration. Whether the consumer's crate depends on
`into-sorted` is handled by the activation mechanism, not a knob: a crate
that does not (and will not) take the dependency disables the rule via
`[perfectionist].disable`. The autofix assumes the dependency is present
or addable — see Implementation notes.

## Out of scope

- **A binding that does not own the `Vec`.** A `&Vec<T>` / `&mut Vec<T>`
  binding can call `sort()` but cannot be consumed by `into_sorted`, which
  takes the `Vec` by value. The initializer's resolved type must be an
  owned `Vec<T>`.
- **Slices and arrays.** `into-sorted`'s `IntoSorted` is implemented for
  any `AsMut<[Item]> + Sized` owner, so an in-place-sorted `[T; N]` is
  theoretically in range, but arrays are rarely bound-then-sorted this way
  and the payoff is marginal. Left out to keep the trigger tied to the
  `Vec` case.
- **A non-adjacent sort.** If a statement sits between the binding and the
  sort, the rule stays silent rather than reason about whether that
  statement observes the binding. Relaxing this to "no intervening *use*
  of the binding" is a possible later extension, but it reintroduces a
  small use-check the strict-adjacency form avoids.

## Implementation notes

- **Trigger discovery — three checks, cheapest first.** Walk
  `StmtKind::Let` and apply, in order:
  1. **Binding mode.** The `let` pattern is a plain by-value `mut`
     binding (`ByRef::No` + `Mutability::Mut`). This is a pure-HIR gate
     that needs no type resolution, so it narrows the search space
     before anything expensive runs — and it is sound as a *filter*,
     because a non-`mut` owned binding could not have compiled (E0596;
     see "The `mut`'s *necessity* is not the trigger" above). It also
     discards the `let ref mut v = vec![…];` trap for free, `ref mut`
     being a by-*reference* binding mode rather than a `mut` one. Keep
     it a filter: it must never grow into a "is the `mut` removable?"
     proof.
  2. **Type.** The *binding* resolves through `cx.typeck_results()` to
     an owned `Vec<T>`. Still required — step 1 is necessary but not
     sufficient, since `let mut v: &mut Vec<T> = buf;` is a by-value
     `mut` binding of a *reference* and must be rejected. Read the
     binding's type, not the initializer expression's.
  3. **Adjacency.** The binding's next sibling statement in the block is
     an inherent-`Vec` `sort*` method call on that binding, in statement
     position with its result discarded.

  The initializer's syntactic form is irrelevant at every step — do
  **not** require a `collect` root or inspect the chain shape.
- **No use-analysis.** The rule reads only the two adjacent statements;
  it never enumerates the binding's later uses. Correctness comes from
  adjacency (nothing observes the intermediate value) plus value-identity
  of the fold, not from proving the `mut` is dead — that is `unused_mut`'s
  job, deliberately left to it.
- **Autofix.** Rewrite the `let … = <init>; binding.sort*(args);` pair
  into `let … = <init>.into_sorted*(args);`, deleting the sort statement
  and adding `use into_sorted::IntoSorted;` / `IntoSortedUnstable` if
  absent. Keep the binding's `mut` exactly as written — `unused_mut`
  removes it when it becomes redundant. **Turbofish only when needed:** if
  `<init>`'s element type was fixed solely by the `let` annotation or the
  now-removed `sort` (the classic case is a bare `collect()`), give it an
  explicit type — e.g. `collect::<Vec<_>>()` — so the owning method
  resolves; an initializer that already carries a concrete `Vec<T>` type
  (`vec![…]`, a typed call) needs no turbofish. `MachineApplicable` when
  the crate already depends on `into-sorted`; otherwise `MaybeIncorrect`,
  since a late pass cannot add the dependency to `Cargo.toml`.
- **Proc-macro suppression.** The primary span is the `sort*` call —
  wider than a bare identifier — so, by the
  [proc-macro suppression convention](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations),
  the built-in
  `report_in_external_macro: false` filter suffices; no
  `hir_in_external_macro` guard or `ui/<rule>_proc_macro.rs` fixture is
  required. Record that reasoning at the span-selection site.
- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

### Difficulty

**Easy–Medium.** Dropping both the `mut`-necessity proof and the
collect-root requirement removes the hard parts. What remains is a
two-adjacent-statement structural match plus one type check (owned
`Vec<T>`), and an autofix that folds a call and, *only for a
type-inferred initializer*, threads a `Vec<_>` turbofish. The cascade
needs no special handling — it is just the same rule (and its sibling)
re-firing on its own output, because the rewritten initializer is again an
owned `Vec` expression.

## Default state

Active by default. The bind-then-in-place-sort shape is a broad,
project-agnostic readability point and the preference has a single
direction. The dependency caveat is handled by `[perfectionist].disable`,
not a config knob.

## Interaction with sibling rules

- [`in-place-dedup`](./in-place-dedup.md) — the deduping half. The two
  **cascade**: each accepts *any* owned-`Vec` initializer and folds the
  in-place operation that immediately follows it, so `collect` → `sort` →
  `dedup` collapses to a single
  `collect().into_sorted().into_deduped()` over successive fixes, source
  order preserved. Neither needs to know about the other's operation; each
  just re-fires on the owned `Vec` the other produced.
- [`itertools-sort-dedup-collect`](./itertools-sort-dedup-collect.md) —
  the itertools spelling (`sorted().dedup().collect()`) of the same end
  state, rewriting *toward* the `collect().into_sorted()` form this rule
  produces, so the two never disagree about the destination.
- **Clippy.** `clippy::needless_collect` flags a *different* anti-pattern
  (collecting only to immediately re-iterate). `unused_mut` is this
  rule's load-bearing partner: it removes the `mut` the fold leaves
  behind, which is why this rule does not attempt that removal itself.
