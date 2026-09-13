# `temporary_dedup_set`

**Source:** review discussion on
[`KSXGitHub/perfectionist#443`](https://github.com/KSXGitHub/perfectionist/pull/443#discussion_r4000812599),
which named the anti-pattern — a *temporary set for deduplication* —
while arguing about a sibling rule's example, and gave both of the
preferred forms below.

## Statement

A set built from a walk and immediately walked back into a sequence is
a deduplicator that costs a whole container. The set is never read as a
set — nothing tests membership in it, nothing keeps it — so the only
thing it contributes is "duplicates are gone", which a vector does with
two method calls and no second allocation.

```rust
// Avoid: the set exists for the length of one expression.
let mut unique: Vec<String> =
    names.into_iter().collect::<HashSet<_>>().into_iter().collect();

// Avoid: the same round trip with the set named.
let deduplicated: HashSet<String> = names.into_iter().collect();
let mut unique: Vec<String> = deduplicated.into_iter().collect();

// Prefer:
let mut unique: Vec<String> = names.into_iter().collect();
unique.sort();
unique.dedup();
```

The functional form says the same thing in one expression, through
[`into-sorted`](https://docs.rs/into-sorted) and
[`into-deduped`](https://docs.rs/into-deduped) — `into_deduped` drops
*consecutive* equal elements, the way `Vec::dedup` does, so it follows
a sort:

```rust
use into_deduped::IntoDeduped;
use into_sorted::IntoSorted;

let unique = names
    .into_iter()
    .collect::<Vec<String>>()
    .into_sorted()
    .into_deduped();
```

Both std set types round-trip this way, and the rule treats them
uniformly:

| Anti-pattern                            | What the set contributes              | What replaces it |
|-----------------------------------------|---------------------------------------|------------------|
| `.collect::<HashSet<_>>().into_iter()`  | deduplication, and an arbitrary order | `sort` + `dedup` |
| `.collect::<BTreeSet<_>>().into_iter()` | deduplication, and a sorted order     | `sort` + `dedup` |

The `BTreeSet` row is the stronger case: the vector it produces is
already the vector `sort` + `dedup` produces, so the rewrite changes
nothing a caller can observe. The `HashSet` row trades an order that
varies per run for a sorted one.

## Why restrict this?

This is a stylistic preference, not a correctness issue. The round trip
computes the right set of values, and where the resulting order is
never observed it is a legitimate way to spend memory on speed — see
[When the set is the right tool](#when-the-set-is-the-right-tool),
which is the part of this file that decides whether the rule is worth
enabling in a given crate.

The project prefers the vector because:

- **The vector's order is reproducible; the set's is not.** A
  `HashSet` is seeded per instance, so the round trip's output order
  differs between two sets in one process, let alone between runs. Any
  output built from that vector — a printed list, a lockfile, a hash,
  a test assertion — inherits the variation, and the bug it eventually
  causes surfaces far from the expression that caused it. Nothing
  *depends* on the scrambling, which is what makes it worth removing:
  it is a free source of variation.
- **The value is stated once.** The flagged code builds a container
  whose element type is repeated in a turbofish, then throws it away.
  `sort` and `dedup` name the two things being asked for, on the
  vector the code wanted in the first place.
- **One allocation instead of two.** The vector is sorted in place and
  truncated; the set is a second table that is filled, walked, and
  dropped. Where a set is built from a sized iterator it also reserves
  for every *input* element, duplicates included, so the table is
  allocated at full size even when the result is tiny.

## What the measurements show

Measured on x86-64 (rustc 1.99.0-nightly, `-O -C target-cpu=native`)
with a throwaway single-file harness: generate a source vector, hand
each subject its own clone of it five times, keep the best time and
the peak allocation a counting global allocator saw above the input.
Each cell below is that subject's time as a multiple of the
`collect` + `sort` + `dedup` column, which is therefore 1.00×: 2.22×
is twice as slow, 0.34× is three times as fast.

| Workload (input → unique)                  | `HashSet` | `HashSet` + `sort` | `BTreeSet` | `sort` + `dedup` | `sort_unstable` + `dedup` |
|--------------------------------------------|-----------|--------------------|------------|------------------|---------------------------|
| 1M `u64`, all distinct                     | 2.22×     | 3.17×              | 1.34×      | 1.00× (27 ms)    | 0.72×                     |
| 1M `u64`, 1% distinct                      | 2.23×     | 2.07×              | 1.03×      | 1.00× (16 ms)    | 0.71×                     |
| 500k `String` (16 B), all distinct         | 0.34×     | 1.54×              | 1.08×      | 1.00× (144 ms)   | 0.54×                     |
| 500k `String` (216 B, 200 B shared prefix) | 0.23×     | 1.33×              | 1.02×      | 1.00× (339 ms)   | 0.84×                     |
| 200k `String` (4 KiB, shared prefix)       | 0.22×     | 1.32×              | 1.02×      | 1.00× (1070 ms)  | 1.05×                     |

Three findings shape the rule:

1. **The `BTreeSet` round trip never won.** It ran between 0.98× and
   1.34× the vector's time across every workload — never faster
   outside measurement noise — allocated more (9.7 MiB against
   7.6 MiB on the 1M-`u64` case), and produced the identical sorted
   vector. There is no workload in which to prefer it, which is why
   the `BTreeSet` branch is the machine-applicable one.
2. **The `HashSet` round trip wins on elements that are expensive to
   compare, and only while its order goes unused.** Sorting
   `Vec<String>` chases a pointer per comparison; hashing touches each
   string once. Hence 0.22×–0.34× for strings — and 1.32×–1.54× for
   the same code once a `sort` is appended to make the output
   deterministic. For `u64`, where a comparison is a register
   instruction, the round trip is ~2× *slower*.
3. **The set is not the lighter container it looks like.**
   Deduplicating 1M values with 100 distinct ones, the set added
   18 MiB on top of the 8 MiB input vector, because
   `HashSet::from_iter` reserves on the iterator's size hint: the
   table is sized for the input, duplicates included, not for the
   result. The O(unique) footprint the set is reached for appears only
   when the source has no usable size hint — the same workload behind
   a `filter` peaked at a few KiB for the set against the vector's
   8 MiB.

## When the set is the right tool

The rule stays silent on all of these; the last two are the cases for
`#[allow(perfectionist::temporary_dedup_set, reason = "...")]`.

- **The element is not `Ord`.** `Hash + Eq` without an ordering is
  common — an enum that derives neither `PartialOrd` nor `Ord`, a
  struct containing a `HashMap`. `sort` does not compile for it, so
  there is no fix to suggest and the rule does not fire. This gate is
  a trait-resolution check, not a heuristic.
- **The set is read as a set.** A `contains` call, a `len`, an
  `insert` after the fact, a return, a store into a field, a borrow
  that outlives the expression — any of these and the value is not
  temporary, whatever else happens to it.
- **Deduplication preserves the input order.** The
  `seen.insert(item)`-in-a-`retain` idiom, `itertools::unique()`, and
  `indexmap::IndexSet` all keep first-occurrence order, which `sort`
  destroys. They are not this shape — the set is consumed by `insert`
  calls, or the container is order-preserving — and `IndexSet` is not
  in the recognised set-type list for exactly this reason.
- **The walk is not re-collected.** `for name in
  names.into_iter().collect::<HashSet<_>>()` visits unique elements in
  an arbitrary order and never builds a sequence; there is nothing to
  sort and nothing to dedup, so the set is doing a job no vector does
  more cheaply.
- **Comparison is expensive and the order is genuinely unused.** The
  string rows above. The honest remedy here is usually not `sort` but
  to stop discarding the set: keep it, name it, and let the code that
  consumes it say it wants a set. Where a vector really is what the
  caller needs, `#[allow]` with the measurement as the reason.
- **The source is lazy and duplicates dominate.** Deduplicating a
  streamed million lines down to a few hundred holds a few hundred in
  a set against a million in a vector. `#[allow]` it, and note that
  the advantage disappears the moment the source is a `Vec` or any
  other sized iterator.

## What to lint

`LateLintPass`. Type resolution decides every part of the trigger: the
container types, the `Ord` bound, and whether the walk lands in a
sequence.

A **set round trip** is three parts, all of which must hold:

1. **The build.** A `collect::<S>()` (or `S::from_iter(..)`) whose
   receiver implements `Iterator`, where `S` is `HashSet<T, _>` or
   `BTreeSet<T>` — any hasher, so an alias that only swaps it
   (`rustc_hash::FxHashSet`) is covered without configuration — or a
   type named in `extra_set_types`. A set that merely *arrives* (a
   parameter, a field, a function's return) is not a build: converting
   one container to another goes one way, and the sibling
   `perfectionist::collection_round_trip` makes the same distinction
   for the same reason.
2. **The walk.** The set is consumed exactly once, by `into_iter()`,
   `iter()`, `iter().cloned()`, `iter().copied()`, or `drain(..)`, and
   is used for nothing else.
3. **The landing.** The walk, after any number of adapters, is
   collected into a sequence: `Vec<U>`, `VecDeque<U>`, or `Box<[U]>`.

Plus one gate: **`T: Ord`**, resolved against the element type. Without
it the suggestion does not compile, so the rule must not fire.

Two discovery loci, one per form in [Statement](#statement):

- **Chained.** Build, walk, and landing in one expression. Everything
  needed is on the chain's spine.
- **Bound.** `let set: S = ...collect();` followed by a walk of `set`.
  Scan the enclosing body for every use of that local: the rule fires
  only when the single walk is the only one. The scan is body-local —
  a `let`-bound set cannot be reached from another body — so this is
  not the crate-wide search a rule over `static` items would need.

Guard against proc-macro-synthesised nodes per the
[suppression convention](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations),
and add the `ui/temporary_dedup_set_proc_macro.rs` fixture: a derive
that expands to a collect-and-walk chain must not be flagged, since
the consumer cannot edit the expansion.

## Examples

**Avoid:**

```rust
// Chained.
let mut unique: Vec<String> =
    names.into_iter().collect::<HashSet<_>>().into_iter().collect();

// Bound, and the adapters do not change the shape.
let sorted: BTreeSet<&str> = entries.iter().map(Entry::name).collect();
let names: Vec<String> = sorted.into_iter().map(str::to_owned).collect();
```

**Prefer:**

```rust
let mut unique: Vec<String> = names.into_iter().collect();
unique.sort();
unique.dedup();

let mut names: Vec<&str> = entries.iter().map(Entry::name).collect();
names.sort();
names.dedup();
let names: Vec<String> = names.into_iter().map(str::to_owned).collect();
```

**Left alone:**

```rust
// The set is read as a set.
let known: HashSet<&str> = manifest.dependencies.keys().copied().collect();
let (known_deps, unknown): (Vec<_>, Vec<_>) =
    requested.into_iter().partition(|name| known.contains(name));

// The set arrives; this is a conversion, not a round trip.
fn sorted_names(names: HashSet<String>) -> Vec<String> {
    let mut names: Vec<String> = names.into_iter().collect();
    names.sort();
    names
}

// Order-preserving deduplication.
let mut seen = HashSet::new();
names.retain(|name| seen.insert(name.clone()));

// No sequence at the end.
for name in names.into_iter().collect::<HashSet<_>>() {
    eprintln!("{name}");
}
```

## Configuration

```toml
# dylint.toml
["perfectionist::temporary_dedup_set"]

# Set types recognised beyond `std::collections::HashSet` and
# `BTreeSet`, as absolute paths per the leading-`::` convention in
# IMPLEMENTATION_CONVENTIONS.md. Empty by default. An alias that only
# swaps the hasher (`rustc_hash::FxHashSet`) resolves to `HashSet` and
# needs no entry; a wrapper (`ahash::AHashSet`) or a separate
# implementation (`hashbrown::HashSet`) does. An order-preserving set
# (`indexmap::IndexSet`) must not be listed — its round trip keeps
# first-occurrence order, which the suggestion would destroy.
extra_set_types = ["::hashbrown::HashSet"]

# Whether test code is left alone: a chain inside a `#[cfg(test)]`
# module, a `#[test]` function, or an integration-test or benchmark
# target. Defaults to `false`, matching the sibling round-trip rule,
# so a test is held to the same shape as the code it exercises.
exempt_tests = false
```

The *form* of the suggestion — the three statements, or the
`into_sorted().into_deduped()` chain from
[Statement](#statement) — is a `style` knob whose values are
`statements` (the std-only form, the default) and `chained` (which
requires the consumer to depend on both crates). Ship it with the
second rewrite, not before: a one-variant `style` enum carries no
information, which is the call `perfectionist::overly_long_print_macro`
already made for its own pending rewrite.

## Implementation notes

- **Shared predicate with the sibling.** The "is this a round trip
  through a set that `sort` + `dedup` replaces?" test is needed by
  `perfectionist::collection_round_trip` too, to decide when to stand
  down (see below). Factor it into a crate-internal module rather than
  duplicating the type checks in both rules, per
  [`CLAUDE.md`](../CLAUDE.md#one-rule-per-file-one-config-per-rule).
- **Autofix, chained form.** Fire the machine-applicable rewrite only
  where the round trip is a `let` initializer, which is where it
  occurs in practice: replace the initializer with the collect that
  produced the set's input, then insert `sort` and `dedup` statements
  after it, adding `mut` to the binding if it lacks one. In any other
  expression position the rewrite needs a block, so emit the help text
  without a suggestion.
- **Autofix, bound form.** Delete the `let` that builds the set,
  re-point the second binding's initializer at the first's, and append
  the two statements. The use scan has already proved the name is dead
  after the walk.
- **`sort` or `sort_unstable`.** Suggest `sort_unstable` when the
  element type is a primitive — equal primitives are
  indistinguishable, it was the fastest subject on the `u64` rows
  above, it allocates nothing where the stable sort allocates scratch,
  and
  `clippy::stable_sort_primitive` would otherwise flag the suggestion
  the moment the consumer applied it. For everything else suggest the
  stable `sort`, which keeps the first of each group of equal elements
  — the element a set keeps too.
- **Dropping a needless clone.** Where the walk is `iter().cloned()`
  or `iter().copied()`, the rewrite drops it: the set was about to be
  dropped, so the elements can be moved. Say so in the diagnostic
  rather than silently changing the chain.
- **Applicability.** `MachineApplicable` for the `BTreeSet` branch,
  whose output is unchanged. `MaybeIncorrect` for the `HashSet`
  branch, which replaces an arbitrary order with a sorted one — the
  right call even though nothing may depend on the arbitrary order,
  because the rewrite is observable.

### Difficulty

**Medium.** The chained form is a spine walk plus three type checks
and a trait-bound query, all of which the late pass has to hand. The
bound form adds a body-local use scan, and that scan is where a wrong
implementation false-positives: a `contains` call, a borrow that
escapes into a closure, or a second walk all have to disqualify the
set. A conservative first cut ships the chained form alone, which
needs no scan at all, and adds the bound form once the disqualifying
uses have fixtures.

## Default state

Active by default. The shape is narrow, the `Ord` gate removes the one
case with no fix, and the remaining exceptions are performance
trade-offs a crate states once with `#[allow]` and a reason. A crate
whose hot paths are full of the string workload above — where the
measurement favours the set by 3–4× as long as the order stays unused
— is the crate that turns the rule off in `[perfectionist].disable`.

## Interaction with clippy and sibling rules

- **`clippy::needless_collect` does not cover this.** It matches
  `Vec`, `VecDeque`, `LinkedList`, and `BinaryHeap` only — dropping a
  set's `collect` would change which elements survive, so it stays out
  of scope there. This rule keeps the deduplication and changes how it
  is spelled, which is why it can cover the types Clippy's cannot.
- **`clippy::stable_sort_primitive`** decides the `sort` /
  `sort_unstable` choice in the suggestion; see the implementation
  notes.
- **`perfectionist::collection_round_trip`
  ([`KSXGitHub/perfectionist#443`](https://github.com/KSXGitHub/perfectionist/pull/443))
  stands down where this rule fires.** It flags the chained form of
  *any* collection round trip and suggests naming the intermediate —
  which, for a set, produces the bound form this rule flags. Left
  alone, the two rules hand the code back and forth and report one
  expression twice. The precedence falls out of the trigger: this rule
  fires only where `sort` + `dedup` is available, so the sibling stays
  silent on exactly those expressions and keeps the ones where naming
  the intermediate *is* the fix — a round trip through a `Vec`, or
  through a set whose element is not `Ord`.
- **`perfectionist::overly_long_method_chain`** measures length; this
  measures shape. A two-line round trip is short and still flagged
  here; a nine-call chain with no round trip is flagged there and not
  here.
- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
