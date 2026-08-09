# `in_place_dedup`

**Source:** [`KSXGitHub/perfectionist#308`](https://github.com/KSXGitHub/perfectionist/issues/308),
which proposes preferring the
[`into-sorted`](https://crates.io/crates/into-sorted) and
[`into-deduped`](https://crates.io/crates/into-deduped) crates — both by
this project's author — over the in-place `Vec::sort*` / `Vec::dedup*`
mutation of a freshly-bound `Vec`. This rule covers the deduping half; its
sibling [`in-place-sort`](./in-place-sort.md) covers the sorting half. The
two are deliberately parallel and **cascade** (see
[Interaction](#interaction-with-sibling-rules)).

## Statement

A `Vec` is bound by value and the **very next statement** deduplicates it
in place — `Vec::dedup`, `Vec::dedup_by`, or `Vec::dedup_by_key`. Because
the in-place dedup takes `&mut self`, the binding has to be `mut`. The
owning dedup methods from
[`into-deduped`](https://crates.io/crates/into-deduped) take the `Vec` by
value and return it deduplicated, so the dedup folds back into the
initializer:

```rust
// Avoid: a separate `mut` binding, deduped in place on the next line.
let mut ids: Vec<Id> = sorted_rows.iter().map(Row::id).collect();
ids.dedup();
ids

// Prefer: the dedup folds into the initializer.
use into_deduped::IntoDeduped;

let ids = sorted_rows.iter().map(Row::id).collect::<Vec<_>>().into_deduped();
```

The binding's initializer can be **any** owned `Vec` expression — a
`collect()`, a `vec![…]` literal, a `Vec`-returning call, a moved
binding. Where the `Vec` comes from does not matter; the fold only needs
the initializer to be a `Vec` value the binding owns, and it is
value-identical in every case:

```rust
// All three fold the same way.
let mut a = load_ids();            a.dedup();   // → load_ids().into_deduped()
let mut b = vec![1, 1, 2];         b.dedup();   // → vec![1, 1, 2].into_deduped()
let mut c: Vec<_> = xs.collect();  c.dedup();   // → xs.collect::<Vec<_>>().into_deduped()
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

### The `mut`'s *necessity* is not the trigger — its *presence* is guaranteed

Two questions about the `mut` are easy to conflate; the rule takes
opposite stances on them.

**Presence — guaranteed, and safe to rely on.** `Vec::dedup*` takes
`&mut self`, so the receiver place must be mutable, and for a binding
that *owns* its `Vec` by value the only way to get that is `let mut`.
rustc rejects the alternative outright: `let v = vec![1, 1, 2];
v.dedup();` is E0596, "cannot borrow `v` as mutable, as it is not
declared as mutable". Every binding this rule can match is therefore
already `mut`, and the implementation may lean on that — the binding mode
is a sound, cheap pre-filter (skip a non-`mut` binding before paying for
type resolution), and the autofix keeps the `mut` as written.

**Necessity — never analysed.** Whether the `mut` is *still* needed once
the dedup folds away is a different question, and proving it costs a
borrow walk over every later use of the binding. The rule does not do
that, and a future implementation must not make it a trigger condition,
because two facts make the proof redundant:

- Folding the dedup into the initializer is **value-identical**: the
  binding holds the same deduplicated `Vec` from that point on, so the
  rewrite is correct no matter how `vec` is used afterward.
- rustc's built-in `unused_mut` already answers the question after the
  fold. If nothing else mutates the binding, it fires and offers to drop
  the `mut`; if something does (a later `push`), the `mut` stays and is
  correct.

So this rule keeps the `mut` as-written in its own suggestion and lets
`unused_mut` take it from there. A binding that is deduped and *then*
`push`ed is therefore still flagged — the dedup folds in, while the `mut`
and the `push` remain (and `unused_mut` correctly stays quiet).

> [!IMPORTANT]
> **The presence guarantee is scoped to by-value bindings.** It holds
> *because* the trigger is restricted to a binding that owns its `Vec`
> (see [What to lint](#what-to-lint)). A `&mut Vec<T>` binding needs no
> `mut` of its own — `let v: &mut Vec<Id> = buf; v.dedup();` compiles,
> because the mutability rides on the reference rather than the binding.
> Such bindings are out of scope anyway (`into_deduped` consumes by
> value), but the check must read the **binding's** type, not the
> initializer expression's: `let ref mut v = vec![1, 1, 2];` has an
> initializer of type `Vec<i32>` and a binding of type `&mut Vec<i32>`,
> with no `mut` binding mode in sight.

## Why restrict this?

This is a stylistic preference, not a correctness issue. Binding a `Vec`
and deduping it in place compiles and produces exactly the right value.
The project prefers the owning form because:

- **The dedup stays in the expression.** `<init>.into_deduped()` reads as
  one pipeline; the `let mut` / `dedup` / use form splits it across a name
  whose only job is to host the mutation.
- **The `mut` usually disappears.** Once the in-place dedup is folded out,
  `unused_mut` clears the now-redundant `mut`, so the bind-then-dedup
  idiom stops minting `mut` bindings that are never mutated again.
- **No window holding a not-yet-deduped value.** Between the binding and
  the `dedup` the binding holds a `Vec` that still has duplicates; a later
  edit that reads it there is silently wrong. The chain has no such
  window — and the adjacency requirement below guarantees no such read
  exists today, while the chain form keeps it that way under future edits.

## What to lint

`LateLintPass`. Type resolution is required to confirm the receiver is a
`Vec<T>` the binding owns and the method is the inherent in-place `dedup*`
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
2. **The immediately following statement dedups it in place.** The *next*
   statement after the `let` is `binding.dedup*(args);` in statement
   position with its `()` result discarded, and there is nothing between
   the two statements. This strict adjacency is the simplification that
   replaces dataflow analysis: with no statement in between, nothing can
   observe the intermediate (not-yet-deduped) value, so folding the dedup
   into the initializer cannot change behaviour.

Emit on the `dedup*` call; the autofix folds it into the initializer.

### The combined `sort` + `dedup` sequence

A bind → `sort` → `dedup` run (canonically `collect` → `sort` → `dedup`,
"sort, then drop the now-adjacent duplicates") is the case a per-call,
`mut`-necessity-based design gets wrong: each rule skips it because the
binding is still mutated by the *other* operation. Here it falls out of
the cascade. On the original three statements **only the sort sibling
fires** (the statement after the `let` is the sort, not the dedup, so
*this* rule does not match yet). Once the sort has folded:

```rust
let mut v = iter.collect::<Vec<_>>().into_sorted();
v.dedup();
```

the initializer is still an owned `Vec` (a chain now ending in
`into_sorted()`) and the next statement is the dedup, so this rule now
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

### A `vec!` literal — also flagged

**Avoid:**

```rust
let mut xs = vec![1, 1, 2, 3, 3];
xs.dedup();
```

**Prefer:** the source is a literal, not a collect, but the fold is the
same (and needs no turbofish — `vec![…]` already has a concrete type) —

```rust
let xs = vec![1, 1, 2, 3, 3].into_deduped();
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
// Something runs between the binding and the dedup, so adjacency fails
// and the rule stays silent (conservative — see "What to lint").
let mut ids: Vec<Id> = rows.iter().map(Row::id).collect();
log::debug!("collected {} ids", ids.len());
ids.dedup();
```

### Not flagged — the binding does not own the `Vec`

```rust
// `v` is a `&mut Vec<_>`, not an owned `Vec`. `into_deduped` consumes the
// `Vec` by value, which a reference cannot provide, so this is out of
// scope (see "Out of scope").
let v: &mut Vec<Id> = &mut buffer;
v.dedup();
```

## Configuration

```toml
# dylint.toml
#
# Active by default. The rule has a single direction (prefer the owning
# `into_deduped*` method) and no per-method toggle.
[perfectionist::in_place_dedup]
```

The rule ships no configuration. Whether the consumer's crate depends on
`into-deduped` is handled by the activation mechanism, not a knob: a
crate that does not take the dependency disables the rule via
`[perfectionist].disable`. The autofix assumes the dependency is present
or addable — see Implementation notes.

## Out of scope

- **A binding that does not own the `Vec`.** A `&Vec<T>` / `&mut Vec<T>`
  binding can call `dedup()` but cannot be consumed by `into_deduped`,
  which takes the `Vec` by value. The initializer's resolved type must be
  an owned `Vec<T>`.
- **Non-consecutive deduplication.** `Vec::dedup*` and `into_deduped*`
  drop only *consecutive* duplicates; the "remove every duplicate
  regardless of position" operation is `itertools::Itertools::unique` /
  `unique_by`, backed by a hash set, with **no** `into-deduped`
  equivalent. This rule must not suggest `into_deduped` for a `unique`
  call — the semantics differ — and
  [`itertools-sort-dedup-collect`](./itertools-sort-dedup-collect.md)
  likewise excludes `unique*`.
- **A non-adjacent dedup.** If a statement sits between the binding and
  the dedup, the rule stays silent rather than reason about whether that
  statement observes the binding. Relaxing this is a possible later
  extension, but it reintroduces a small use-check the strict-adjacency
  form avoids.

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
     an inherent-`Vec` `dedup*` method call on that binding, in statement
     position with its result discarded.

  The initializer's syntactic form is irrelevant at every step — do
  **not** require a `collect` root or inspect the chain shape.
- **No use-analysis.** The rule reads only the two adjacent statements;
  it never enumerates the binding's later uses. Correctness comes from
  adjacency (nothing observes the intermediate value) plus value-identity
  of the fold, not from proving the `mut` is dead — that is `unused_mut`'s
  job, deliberately left to it.
- **Autofix.** Rewrite the `let … = <init>; binding.dedup*(args);` pair
  into `let … = <init>.into_deduped*(args);`, deleting the dedup statement
  and adding `use into_deduped::IntoDeduped;` if absent. Keep the
  binding's `mut` exactly as written — `unused_mut` removes it when it
  becomes redundant. **Turbofish only when needed:** if `<init>`'s element
  type was fixed solely by the `let` annotation or the now-removed `dedup`
  (the classic case is a bare `collect()`), give it an explicit type —
  e.g. `collect::<Vec<_>>()` — so the owning method resolves; an
  initializer that already carries a concrete `Vec<T>` type (`vec![…]`, a
  typed call) needs no turbofish. `MachineApplicable` when the crate
  already depends on `into-deduped`; otherwise `MaybeIncorrect`, since a
  late pass cannot add the dependency to `Cargo.toml`.
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
pair. Dropping both the `mut`-necessity proof and the collect-root
requirement removes the hard parts; what remains is a
two-adjacent-statement match, one type check (owned `Vec<T>`), and a
call-folding autofix (with a `Vec<_>` turbofish threaded *only* for a
type-inferred initializer). The cascade needs no special handling — it is
just the rule re-firing on the owned `Vec` its sort sibling produced.

## Default state

Active by default. The bind-then-in-place-dedup shape is a broad,
project-agnostic readability point with a single-direction preference.
The dependency caveat is handled by `[perfectionist].disable`, not a
config knob.

## Interaction with sibling rules

- [`in-place-sort`](./in-place-sort.md) — the sorting half. The two
  **cascade**: each accepts *any* owned-`Vec` initializer and folds the
  in-place operation that immediately follows it, so `collect` → `sort` →
  `dedup` collapses to a single
  `collect().into_sorted().into_deduped()` over successive fixes, source
  order preserved. Neither needs to know about the other's operation; each
  just re-fires on the owned `Vec` the other produced.
- [`itertools-sort-dedup-collect`](./itertools-sort-dedup-collect.md) —
  the itertools spelling, rewriting *toward* the `collect().into_deduped()`
  form this rule produces. Its exclusion of `itertools::unique*` mirrors
  this rule's: non-consecutive deduplication has no `into-deduped`
  equivalent.
- **Clippy.** There is no `clippy::manual_into_deduped` equivalent;
  `unused_mut` is this rule's load-bearing partner, removing the `mut` the
  fold leaves behind, which is why this rule does not attempt that
  removal itself.
