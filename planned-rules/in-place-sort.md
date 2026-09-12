# `in_place_sort`

**Source:** [`KSXGitHub/perfectionist#308`](https://github.com/KSXGitHub/perfectionist/issues/308),
which proposes preferring the
[`into-sorted`](https://crates.io/crates/into-sorted) and
[`into-deduped`](https://crates.io/crates/into-deduped) crates — both by
this project's author — over an in-place `sort*` / `dedup*` mutation of a
freshly-bound value. This rule covers the sorting half; its sibling
[`in-place-dedup`](./in-place-dedup.md) covers the deduping half. The two
are **not** symmetric: `sort*` are slice methods, so this rule fires on
any sortable-slice owner, whereas `dedup` is a `Vec`-only method. Where
both apply — a `Vec` sorted then deduped — they **cascade** (see
[Interaction](#interaction-with-sibling-rules)).

## Statement

A value is bound and the **very next statement** sorts it in place with a
slice `sort*` method — `sort`, `sort_by`, `sort_by_key`,
`sort_by_cached_key`, or a `sort_unstable*` counterpart. These live on
`[T]` and are reachable on anything that coerces to `&mut [T]`. The owning
methods from [`into-sorted`](https://crates.io/crates/into-sorted) take
the value **by value** and return it sorted — `into_sorted(self) -> Self`
— so the sort folds back into the initializer with the binding's type
unchanged:

```rust
// Avoid: a separate `mut` binding, sorted in place on the next line.
let mut names: Vec<Name> = people.iter().map(Person::name).collect();
names.sort();
names

// Prefer: the sort folds into the initializer.
use into_sorted::IntoSorted;

let names = people.iter().map(Person::name).collect::<Vec<_>>().into_sorted();
```

**Any sortable-slice owner qualifies, not just `Vec`.** `into-sorted`'s
trait is implemented for every `AsMut<[Item]> + Sized` type, and
`into_sorted` returns that same type sorted, so the fold is
value-identical for each:

```rust
let mut v: Vec<_> = xs.collect();  v.sort();   // → xs.collect::<Vec<_>>().into_sorted()
let mut a = [3, 1, 2];             a.sort();   // → [3, 1, 2].into_sorted()        // [T; N]
let mut b = boxed_slice();         b.sort();   // → boxed_slice().into_sorted()    // Box<[T]>
let s: &mut [i32] = buf;           s.sort();   // → buf.into_sorted()              // &mut [T]
```

The last line is the tell that this is **not** a `mut`-elimination rule at
heart: a `&mut [T]` binding needs no `mut` of its own (see
[The `mut` is not the trigger](#the-mut-is-not-the-trigger)).

The method-name mapping is one-for-one and independent of the owner type:

| In-place (slice `sort*`)  | Owning ([`into-sorted`](https://crates.io/crates/into-sorted)) |
|---------------------------|----------------------------------------------------------------|
| `sort()`                  | `into_sorted()`                                                |
| `sort_by(f)`              | `into_sorted_by(f)`                                            |
| `sort_by_key(k)`          | `into_sorted_by_key(k)`                                        |
| `sort_by_cached_key(k)`   | `into_sorted_by_cached_key(k)`                                 |
| `sort_unstable()`         | `into_sorted_unstable()`                                       |
| `sort_unstable_by(f)`     | `into_sorted_unstable_by(f)`                                   |
| `sort_unstable_by_key(k)` | `into_sorted_unstable_by_key(k)`                               |

Each method consumes the owner and returns it sorted (`-> Self`). The
stable `into_sorted*` methods need the crate's `alloc` feature; the
`into_sorted_unstable*` methods do not.

### The `mut` is not the trigger

`sort*` needs a mutable slice, so the receiver must be a mutable place —
but *how* it becomes mutable depends on the owner, and the rule keys on
neither the `mut` keyword nor the mutation:

- **Owned owner** (`Vec<T>`, `[T; N]`, `Box<[T]>`, …). The only way to
  call `sort*` is a `let mut` binding — `let v = vec![3, 1, 2];
  v.sort();` is E0596, "cannot borrow `v` as mutable". So an owned
  receiver is always `mut`, and once the sort folds away the `mut` is
  usually idle; `unused_mut` then clears it.
- **Borrowed owner** (`&mut [T]`, `&mut Vec<T>`). The mutability rides on
  the reference, so the binding needs no `mut` — `let s: &mut [i32] = buf;
  s.sort();` compiles. There is no `mut` to clear; the only gain is
  keeping the sort in the expression.

Because the `mut` is not uniform across owners, **the trigger cannot gate
on the binding mode** — it gates on the receiver's type (see
[What to lint](#what-to-lint)). And whether any `mut` is still *needed*
after the fold is a question the rule deliberately **never** asks: proving
it would cost a borrow walk over every later use, and two facts make the
proof redundant —

- Folding the sort into the initializer is **value-identical**: the
  binding holds the same sorted value from that point on, so the rewrite
  is correct no matter how the binding is used afterward.
- rustc's built-in `unused_mut` already answers it after the fold — if
  nothing else mutates the binding it offers to drop the `mut`; if
  something does (a later `push`), the `mut` stays and is correct.

So the rule keeps whatever `mut` is written and folds; `unused_mut`
finishes the owned case. A binding that is sorted and *then* `push`ed is
still flagged — the sort folds in, the `mut` and the `push` remain, and
`unused_mut` correctly stays quiet. A future implementation **must not**
turn "is the `mut` removable?" into a trigger condition.

> [!IMPORTANT]
> **Gate on the binding's resolved type, not the `mut` keyword.** Borrowed
> owners sort with no `mut` binding, so the keyword filters nothing; the
> trigger is a **by-value** binding (`ByRef::No`) whose type implements
> `AsMut<[Item]> + Sized` — exactly what `into_sorted` accepts. That rules
> out two cases a `mut`-based gate would mishandle: a `ref` / `ref mut`
> binding, whose binding is a *reference to* the initializer rather than
> the sortable value itself, so there is no by-value owner to fold
> `into_sorted` onto; and a receiver that reaches `sort` only through
> `DerefMut` without also implementing `AsMut<[Item]>`, where the fold
> would not compile.

## Why restrict this?

This is a stylistic preference, not a correctness issue. Binding a value
and sorting it in place compiles and produces exactly the right result.
The project prefers the owning form because:

- **The sort stays in the expression.** `<init>.into_sorted()` reads as
  one pipeline; the `let` / `sort` / use form splits it across a name
  whose only job is to host the mutation.
- **The `mut` usually disappears — for an owned receiver.** Once the
  in-place sort is folded out, `unused_mut` clears the now-redundant
  `mut`, so the bind-then-sort idiom stops minting `mut` bindings that are
  never mutated again. (A `&mut [T]` receiver has no `mut` to clear; there
  the gain is only the expression form.)
- **No window holding an unsorted value.** Between the binding and the
  `sort` the binding names an *un*-sorted value; a later edit that reads
  it there is silently wrong. The chain has no such window — and the
  adjacency requirement below guarantees no such read exists today, while
  the chain form keeps it that way under future edits.

## What to lint

`LateLintPass`. Type resolution is required — to confirm the receiver's
type implements `AsMut<[Item]> + Sized` (so `into_sorted` applies) and
that the method is a slice `sort*` reached on it (not a same-named method
on an unrelated type) — so this is a late pass.

Fire when **both** hold:

1. **A by-value `let` binding of a sortable-slice owner.** The binding is
   `ByRef::No`, and its type resolves through `cx.typeck_results()` to a
   `T: AsMut<[Item]> + Sized` — `Vec<T>`, `[T; N]`, `Box<[T]>`,
   `&mut [T]`, `&mut Vec<T>`, or any other such owner. The *syntactic*
   source is irrelevant — a `collect()`, a `vec![…]`, an array literal, a
   value-returning call, or a chain already ending in an owning
   `into_sorted*` / `into_deduped*` method all qualify. (The last is what
   lets the sort/dedup pair [cascade](#the-combined-sort--dedup-sequence)
   on a `Vec`: `into_sorted` returns the same owner type, so the chain is
   itself just another sortable-slice initializer.)
2. **The immediately following statement sorts it in place.** The *next*
   statement after the `let` is `binding.sort*(args);` in statement
   position with its `()` result discarded, and there is nothing between
   the two statements. This strict adjacency replaces dataflow analysis:
   with no statement between, nothing can observe the intermediate
   (unsorted) value — for an owned owner nothing *can* (the binding owns
   it); for a `&mut` owner the unique borrow blocks other access — so
   folding the sort into the initializer cannot change behaviour.

Emit on the `sort*` call; the autofix folds it into the initializer.

### The combined `sort` + `dedup` sequence

A bind → `sort` → `dedup` run (canonically `collect` → `sort` → `dedup`,
"sort, then drop the now-adjacent duplicates") is the case a per-call,
`mut`-necessity-based design gets wrong: each rule skips it because the
binding is still mutated by the *other* operation. It only arises on a
`Vec` (the receiver of `dedup`), and here it falls out of the cascade with
no special handling. On the original three statements **only this rule
fires**: its trigger is *a `let`-bound sortable owner immediately followed
by a sort*, while the dedup sibling's trigger — *a `let`-bound `Vec`
immediately followed by a dedup* — does not match, because the statement
after the `let` is the sort, not the dedup. After the sort folds:

```rust
let mut v = iter.collect::<Vec<_>>().into_sorted();
v.dedup();
```

the initializer is still a `Vec` (a chain now ending in `into_sorted()`)
and the next statement is the dedup, so the dedup sibling matches and
folds in turn:

```rust
let v = iter.collect::<Vec<_>>().into_sorted().into_deduped();
```

(then `unused_mut` drops the `mut`). Under `cargo dylint --fix` this
resolves over successive iterations; under a plain run the author sees the
sort warning, applies it, then sees the dedup warning. Source order is
preserved because each rule appends its method only when its operation is
the statement immediately following the binding.

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

### An array or a `vec!` literal — also flagged

**Avoid:**

```rust
let mut xs = vec![3, 1, 2];
xs.sort();

let mut ys = [3, 1, 2];   // [i32; 3] — a slice owner just like `Vec`
ys.sort();
```

**Prefer:** neither source is a collect, and both already have a concrete
type, so no turbofish is needed —

```rust
let xs = vec![3, 1, 2].into_sorted();
let ys = [3, 1, 2].into_sorted();
```

### A `&mut [T]` slice — flagged, though there was no `mut`

**Avoid:** uncommon, but in scope — the mutability is in the reference, so
the binding is not `mut` and there is nothing for `unused_mut` to remove;
the only change is that the sort joins the expression.

```rust
fn normalize(buf: &mut [i32]) -> &mut [i32] {
    let s: &mut [i32] = buf;
    s.sort();
    s
}
```

**Prefer:**

```rust
fn normalize(buf: &mut [i32]) -> &mut [i32] {
    buf.into_sorted()
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
// Something runs between the binding and the sort, so adjacency fails and
// the rule stays silent (conservative — see "What to lint").
let mut names: Vec<Name> = people.iter().map(Person::name).collect();
log::debug!("collected {} names", names.len());
names.sort();
```

### Not flagged — not a `let`-bound value

```rust
// `data` is a parameter, not a `let` binding immediately preceding the
// sort, so there is no initializer to fold the sort into.
fn normalize(data: &mut [i32]) {
    data.sort();
    // ...
}
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

- **A `ref` / `ref mut` pattern binding** (distinct from a by-value
  binding *of* a reference). `let s: &mut [T] = buf; s.sort();` is in
  scope — a by-value binding whose value is a `&mut [T]`, folded to
  `buf.into_sorted()`. A `ref mut` binding is not: it binds *by reference*
  to a field or element of the scrutinee — a top-level `let ref mut v =
  …`, or a destructuring `let` / `match` / `if let` — so it has **no
  initializer to fold into**. (Plain `ref` binds `&T` and cannot be a
  `sort*` receiver at all.) The everyday shape — a `ref mut` field sorted
  in a `match` arm — is idiomatic in place with no cleaner form; the one
  foldable shape, a destructured *literal aggregate of owned values* each
  sorted, needs multi-site, type-aware machinery this rule avoids by
  design (several sorts rather than strict two-statement adjacency, and a
  binding-type change to propagate), so it is left to a possible
  **separate** rule. (`clippy::toplevel_ref_arg` already steers a
  top-level `let ref mut v = E` toward `let v = &mut E`, the `&mut` form
  this rule *does* cover.)
- **A receiver sortable only through `DerefMut`.** `into_sorted` needs
  `AsMut<[Item]>`; a smart pointer that reaches `[T]` through `DerefMut`
  but does not implement `AsMut<[Item]>` can call `.sort()`, yet the
  folded `into_sorted` would not compile. The rule confirms the trait
  applies and stays silent otherwise.
- **A non-adjacent sort.** If a statement sits between the binding and the
  sort, the rule stays silent rather than reason about whether that
  statement observes the binding. Relaxing this to "no intervening *use*
  of the binding" is a possible later extension, but it reintroduces a
  small use-check the strict-adjacency form avoids.

## Implementation notes

- **Trigger discovery.** Walk `StmtKind::Let` with a by-value binding
  (`ByRef::No`); resolve the binding's type through `cx.typeck_results()`
  and confirm it implements `AsMut<[Item]> + Sized` — the same predicate
  that decides `into_sorted` applies (a trait-resolution check, e.g. via
  `clippy_utils`' `implements_trait`). Do **not** gate on the `mut`
  keyword: an owned receiver is `mut`, a `&mut [T]` / `&mut Vec<T>`
  receiver is not, and both are in scope. Then confirm the binding's next
  sibling statement is a slice `sort*` call (`<[T]>::sort*`, matched by
  `DefId`, reached on that binding), in statement position with its result
  discarded. The initializer's syntactic form is irrelevant — do not
  require a `collect` root.
- **No use-analysis.** The rule reads only the two adjacent statements; it
  never enumerates the binding's later uses. Correctness comes from
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
  resolves; an initializer that already carries a concrete type (an array
  literal, a `vec![…]`, a typed call) needs no turbofish.
  `MachineApplicable` when the crate already depends on `into-sorted`;
  otherwise `MaybeIncorrect`, since a late pass cannot add the dependency
  to `Cargo.toml`.
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

**Easy–Medium.** The structural match (two adjacent statements) is simple;
the care goes into the type predicate — a trait-resolution check that the
receiver is `AsMut<[Item]> + Sized` (so `into_sorted` applies), covering
`Vec` / `[T; N]` / `Box<[T]>` / `&mut [T]` alike — and the autofix, which
folds a call and threads a `Vec<_>` turbofish only for a type-inferred
`collect()`. The cascade needs no special handling: `into_sorted` returns
the same owner type, so a rewritten initializer is again a sortable-slice
value the rule (or, on a `Vec`, its dedup sibling) can re-fire on.

## Default state

Active by default. The bind-then-in-place-sort shape is a broad,
project-agnostic readability point and the preference has a single
direction. The dependency caveat is handled by `[perfectionist].disable`,
not a config knob.

## Interaction with sibling rules

- [`in-place-dedup`](./in-place-dedup.md) — the deduping half, and the
  **narrower** of the two: `dedup` is a `Vec`-only method, so that rule
  fires only on an owned `Vec`, while this rule fires on any
  sortable-slice owner. Where both apply — a `Vec` sorted then deduped —
  they **cascade**: `collect` → `sort` → `dedup` collapses to a single
  `collect().into_sorted().into_deduped()` over successive fixes, source
  order preserved, because `into_sorted` returns the `Vec` and the dedup
  rule re-fires on it. On a non-`Vec` owner (an array, a `&mut [T]`) only
  this rule fires.
- [`itertools-sort-dedup-collect`](./itertools-sort-dedup-collect.md) —
  the itertools spelling (`sorted().dedup().collect()`) of the same end
  state, rewriting *toward* the `collect().into_sorted()` form this rule
  produces, so the two never disagree about the destination.
- **Clippy.** `clippy::needless_collect` flags a *different* anti-pattern
  (collecting only to immediately re-iterate). `unused_mut` is this
  rule's load-bearing partner *for owned receivers*: it removes the `mut`
  the fold leaves behind, which is why this rule does not attempt that
  removal itself.
