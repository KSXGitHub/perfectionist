# `in_place_sort_after_collect`

**Source:** [`KSXGitHub/perfectionist#308`](https://github.com/KSXGitHub/perfectionist/issues/308),
which proposes preferring the
[`into-sorted`](https://crates.io/crates/into-sorted) and
[`into-deduped`](https://crates.io/crates/into-deduped) crates — both by
this project's author — over the in-place `Vec::sort*` / `Vec::dedup*`
mutation that forces a binding to be `mut`. This rule covers the sorting
half; its sibling [`in-place-dedup-after-collect`](./in-place-dedup-after-collect.md)
covers the deduping half.

## Statement

An iterator is collected into a `Vec`, the binding is marked `mut`, and
the *only* reason for the `mut` is an immediately-following in-place sort
(`Vec::sort`, `Vec::sort_by`, `Vec::sort_by_key`,
`Vec::sort_by_cached_key`, or their `sort_unstable*` counterparts). The
`mut` would not have been necessary otherwise. The owning sort methods
from [`into-sorted`](https://crates.io/crates/into-sorted) take the `Vec`
by value and return it sorted, so the whole thing stays one expression
and the `mut` binding disappears:

```rust
// Avoid: `mut` exists only so `sort` can run in place.
let mut names: Vec<Name> = people.iter().map(Person::name).collect();
names.sort();
names

// Prefer: the owning sort keeps the value in the method chain.
use into_sorted::IntoSorted;

let names: Vec<Name> = people.iter().map(Person::name).collect();
let names = names.into_sorted();
// …or, collapsed entirely:
let names = people
    .iter()
    .map(Person::name)
    .collect::<Vec<_>>()
    .into_sorted();
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

Every method consumes the `Vec` and returns it sorted, so the value
never leaves the chain. (The stable `into_sorted*` methods need the
crate's `alloc` feature; the `into_sorted_unstable*` methods do not. For
a `Vec` both are available.)

## Why restrict this?

This is a stylistic preference, not a correctness issue. The `mut`
binding followed by an in-place `sort` compiles and produces exactly the
right value. The project prefers the owning form because:

- **The `mut` disappears.** A `mut` binding tells the reader "this value
  changes after it is created"; here it does not, beyond the one sort the
  rule already accounts for. Dropping the `mut` removes a false signal
  and one more name whose mutation the reader must track.
- **The value stays in the expression.** `collect().into_sorted()` reads
  as a single transformation pipeline; the three-statement
  `let mut`/`sort`/use form interrupts it with a name that exists only to
  host the mutation.
- **No room for a use-before-sort bug.** With the in-place form there is
  a window — between the `collect` and the `sort` — in which the binding
  holds an *un*-sorted `Vec`; a later edit that reads it there gets the
  wrong order silently. The owning chain has no such window.

## What to lint

`LateLintPass`. Type resolution is required to confirm the receiver is a
`Vec<T>` and that the method is the in-place `sort*` (not a same-named
method on another type), so this is a late pass, not a token scan.

Fire when **all** of the following hold for a local `let mut` binding:

1. **The initializer is a `collect()` into `Vec`.** The binding's value
   comes from an `Iterator::collect` whose resolved target type is
   `Vec<T>` (turbofished, type-annotated, or inferred). This is the case
   the issue names; the broader "any owned `Vec` expression" (`vec![…]`,
   a function returning `Vec`) is a deliberate out-of-scope extension —
   see [Out of scope](#out-of-scope).
2. **The binding is `mut`.** A non-`mut` binding cannot host the in-place
   `sort`, so there is nothing to simplify.
3. **The next use is an in-place `sort*`.** The first statement that
   mentions the binding is `binding.sort*(…)` — one of the seven methods
   in the table — as a statement-position method call whose return value
   (`()`) is discarded.
4. **The `mut` is needed for nothing else.** After the `sort`, the
   binding is never mutated again: no further `&mut` borrow, no
   reassignment, no other mutating method, no `&mut`-taking call. (Reads,
   shared borrows, and a final move/return are all fine.) This is the
   load-bearing check — it is what proves the `mut` is *only* there for
   the sort. A binding that is sorted and then `push`ed to genuinely
   needs its `mut` and must not fire.

Emit on the `sort*` call (with the binding's `mut` highlighted as a
secondary span) and offer the autofix below.

### Multiple consecutive mutations

`collect` → `sort` → `dedup` is the canonical "sort then remove
duplicates" sequence. Both this rule and
[`in-place-dedup-after-collect`](./in-place-dedup-after-collect.md) see
it, but neither may fire in isolation while a *different* mutation
remains: this rule's check 4 fails because the binding is still mutated
(by `dedup`) after the `sort`, and vice-versa. The two rules coordinate
so that a run of *only* `sort*` / `dedup*` mutations collapses to a
single `collect().into_sorted().into_deduped()` chain; see
[Interaction with sibling rules](#interaction-with-sibling-rules).

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

### Keyed unstable sort

**Avoid:**

```rust
let mut entries: Vec<Entry> = stream.collect();
entries.sort_unstable_by_key(|e| e.priority);
process(entries);
```

**Prefer:**

```rust
let entries = stream.collect::<Vec<_>>().into_sorted_unstable_by_key(|e| e.priority);
process(entries);
```

### Not flagged — the `mut` is needed for more

```rust
// `push` after the sort genuinely needs `mut`; no owning rewrite applies.
let mut names: Vec<Name> = people.iter().map(Person::name).collect();
names.sort();
names.push(Name::sentinel());
```

```rust
// Not a collect: out of scope for this rule (see "Out of scope").
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
`[perfectionist].disable` rather than configuring it away. The autofix
assumes the dependency is present (or addable) — see Implementation
notes.

## Out of scope

- **Owned `Vec`s from a source other than `collect`.** `vec![…].sort()`,
  a `Vec` returned by a helper, or a `Vec` literal bound to `mut` for a
  sort share the same shape and the same owning rewrite, but the issue
  scopes this rule to the *collect-into-`Vec`* case, and the name claims
  no more. Broadening to every owned `Vec` is a candidate follow-up, not
  part of this rule's trigger.
- **Slices and arrays.** `into-sorted`'s `IntoSorted` is implemented for
  any `AsMut<[Item]> + Sized` owner, so a `[T; N]` array sorted in place
  is theoretically in range, but arrays are rarely collected into and the
  `mut`-elision payoff is marginal. Left out to keep the trigger tied to
  the issue's `Vec` case.
- **In-place sorts whose result feeds a `&mut` consumer.** If the sorted
  `Vec` is then handed to something that needs `&mut` anyway, the `mut`
  is not redundant; check 4 already excludes these.

## Implementation notes

- **Trigger discovery.** Walk `StmtKind::Let` bindings with
  `BindingMode::MUT`; confirm the initializer resolves through
  `cx.typeck_results()` to a `Vec<T>` produced by `Iterator::collect`
  (an `ExprKind::MethodCall` whose `DefId` is `Iterator::collect` and
  whose result type is `Vec<_>`). Then scan the enclosing block's
  following statements for the first mention of the binding.
- **The "only mutated by the sort" check.** Reuse a `Mutability`/borrow
  walk over the binding's later uses (the same shape clippy's
  `needless_collect` and the `mut`-detection lints use): a binding is
  eligible only if, after the `sort*` call, every remaining use is a
  shared read or the terminal move. Capture the binding's `HirId` and
  classify each subsequent reference. Be conservative: any use the walk
  cannot prove is a shared read disqualifies the binding.
- **Autofix.** Rewrite the `let mut x: Vec<T> = <iter>.collect(); x.sort*(args);`
  pair into `let x = <iter>.collect::<Vec<_>>().into_sorted*(args);`
  (dropping the `mut`, splicing the same `args`), and add
  `use into_sorted::IntoSorted;` / `IntoSortedUnstable` if absent.
  `MachineApplicable` only when the binding's sole post-sort uses are
  reads/move *and* the crate already depends on `into-sorted` (otherwise
  the fix references an absent crate); downgrade to `MaybeIncorrect`
  when the dependency cannot be confirmed, so `cargo dylint --fix` leaves
  adding it to the author. A late pass cannot edit `Cargo.toml`, so the
  added dependency is a help note, never an applied edit.
- **Proc-macro suppression.** The diagnostic's primary span is the
  `sort*` method call — wider than a bare identifier — so by the
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

**Medium.** Recognising the `collect`-into-`mut`-`Vec`-then-`sort` shape
is a local block walk plus one type check. The work — and the
correctness risk — is concentrated in check 4: proving the `mut` is used
for nothing but the sort requires a careful pass over the binding's later
uses, the same negative-proof that separates a correct implementation
from one that strips a `mut` the code still needs. The autofix's
dependency-presence handling is the other moving part.

## Default state

Active by default. The collect-then-in-place-sort shape is a broad,
project-agnostic readability point and the preference has a single
direction. The one project-level caveat — whether the crate takes the
`into-sorted` dependency — is handled by `[perfectionist].disable`, not a
config knob.

## Interaction with sibling rules

- [`in-place-dedup-after-collect`](./in-place-dedup-after-collect.md) —
  the deduping half of the same `into-sorted` / `into-deduped` proposal.
  On a `collect` → `sort` → `dedup` run, neither rule fires in isolation
  (each one's "nothing else mutates the binding" check is failed by the
  *other's* mutation); the two coordinate to collapse a run of only
  `sort*` / `dedup*` mutations into a single
  `collect().into_sorted().into_deduped()` chain, preserving source
  order of the two operations.
- [`itertools-sort-dedup-collect`](./itertools-sort-dedup-collect.md) —
  addresses the *other* spelling of the same end state: itertools'
  `sorted()` / `dedup()` iterator adaptors terminated by `collect()`.
  That rule rewrites *toward* the `collect().into_sorted()` form this
  rule already targets, so the two never disagree about the destination.
- **Clippy.** There is no `clippy::manual_into_sorted` or equivalent;
  `clippy::needless_collect` flags a *different* anti-pattern (collecting
  only to immediately re-iterate), not the `mut`-elision this rule is
  about.
