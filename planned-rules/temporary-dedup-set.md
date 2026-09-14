# `temporary_dedup_set`

**Source:** review discussion on
[`KSXGitHub/perfectionist#443`](https://github.com/KSXGitHub/perfectionist/pull/443#discussion_r4000812599),
which named the anti-pattern — a *temporary set for deduplication* —
and gave both of the preferred forms below. That pull request proposed
a general round-trip rule whose suggested fix was to *name* the
intermediate collection; it was closed in favour of this file, because
naming a deduplicating set leaves the second form below, which is no
better than the first. There is no general round-trip rule to defer
to, and this one claims only the set case.

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
unique.sort_unstable();
unique.dedup();
```

`sort_unstable` rather than `sort` because the round trip being
replaced has no input order to preserve: a `HashSet` hands back an
arbitrary one, and a `BTreeSet` has already collapsed equal elements.
Stability would guarantee something about an order the flagged code
never had — see the implementation notes for the measurements and the
one element type that argues the other way.

The functional form says the same thing in one expression, through
[`into-sorted`](https://docs.rs/into-sorted) and
[`into-deduped`](https://docs.rs/into-deduped) — `into_deduped` drops
*consecutive* equal elements, the way `Vec::dedup` does, so it follows
a sort:

```rust
use into_deduped::IntoDeduped;
use into_sorted::IntoSortedUnstable;

let unique = names
    .into_iter()
    .collect::<Vec<String>>()
    .into_sorted_unstable()
    .into_deduped();
```

Both std set types round-trip this way, and the rule treats them
uniformly:

| Anti-pattern                            | What the set contributes              | What replaces it          |
|-----------------------------------------|---------------------------------------|---------------------------|
| `.collect::<HashSet<_>>().into_iter()`  | deduplication, and an arbitrary order | `sort_unstable` + `dedup` |
| `.collect::<BTreeSet<_>>().into_iter()` | deduplication, and a sorted order     | `sort_unstable` + `dedup` |

The `BTreeSet` row is the stronger case: the vector it produces is
already the vector `sort_unstable` + `dedup` produces, so the rewrite
changes nothing a caller can observe. The `HashSet` row trades an
order that varies per run for a sorted one.

## Why restrict this?

This is a stylistic preference, not a correctness issue. The round trip
computes the right set of values, and on a large enough input of
pointer-chasing elements whose resulting order nothing reads it is the
faster of the two — see
[When the set is the right tool](#when-the-set-is-the-right-tool),
which is the part of this file that decides whether the rule is worth
enabling in a given crate. For most element types, though, the
preference is free at every size measured: the vector form is also the
quicker one.

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
  `sort_unstable` and `dedup` name the two things being asked for, on
  the vector the code wanted in the first place.
- **One allocation instead of two.** The vector is sorted in place and
  truncated; the set is a second table that is filled, walked, and
  dropped. Where a set is built from a sized iterator it also reserves
  for every *input* element, duplicates included, so the table is
  allocated at full size even when the result is tiny.

## What the measurements show

Each cell below is that subject's time as a multiple of the
`collect` + `sort` + `dedup` column, which is therefore 1.00×: 2.22×
is twice as slow, 0.34× is three times as fast. The element-type
table further down uses `sort_unstable` + `dedup` as its baseline
instead, and says so.

| Workload (input → unique)                  | `HashSet` | `HashSet` + `sort` | `BTreeSet` | `sort` + `dedup` | `sort_unstable` + `dedup` |
|--------------------------------------------|-----------|--------------------|------------|------------------|---------------------------|
| 1M `u64`, all distinct                     | 2.22×     | 3.17×              | 1.34×      | 1.00× (27 ms)    | 0.72×                     |
| 1M `u64`, 1% distinct                      | 2.23×     | 2.07×              | 1.03×      | 1.00× (16 ms)    | 0.71×                     |
| 500k `String` (16 B), all distinct         | 0.34×     | 1.54×              | 1.08×      | 1.00× (144 ms)   | 0.54×                     |
| 500k `String` (216 B, 200 B shared prefix) | 0.23×     | 1.33×              | 1.02×      | 1.00× (339 ms)   | 0.84×                     |
| 200k `String` (4 KiB, shared prefix)       | 0.22×     | 1.32×              | 1.02×      | 1.00× (1070 ms)  | 1.05×                     |

The findings that shape the rule:

1. **The `BTreeSet` round trip never won.** It ran between 0.98× and
   1.34× the vector's time across every workload — never faster
   outside measurement noise — allocated more (9.7 MiB against
   7.6 MiB on the 1M-`u64` case), and produced the identical sorted
   vector. There is no workload in which to prefer it, which is why
   the `BTreeSet` branch is the machine-applicable one. "Identical"
   rests on `Ord` agreeing with `Eq`, which the `Ord` trait requires:
   a `BTreeSet` decides duplicates by `cmp`, `dedup` decides them by
   `==`, so a type whose `cmp` returns `Equal` for values that are not
   `==` would keep more elements after the rewrite than before.
   `clippy::derive_ord_xor_partial_ord` and
   `clippy::derived_hash_with_manual_eq` police that contract; this
   rule assumes it, and says so rather than re-deriving it.
2. **The `HashSet` round trip wins on elements that are expensive to
   compare — which is fewer types than it sounds — and only while its
   order goes unused.** Rust's `String` carries no small-string
   optimisation, so sorting chases a pointer per comparison where
   hashing touches each string once. Hence
   0.22×–0.34× on these synthetic strings — and 1.32×–1.54× for the
   same code once a `sort` is appended to make the output
   deterministic, which is the comparison to make as soon as anything
   observes the order. For `u64`, where a comparison is a register
   instruction, the round trip is ~2× *slower*.
3. **The synthetic strings above overstate it; real names are
   cheaper to sort.** Repeating the measurement on package names —
   4.44M npm names (mean 20 B, median 18 B) and the 1000
   most-downloaded crates.io names (mean 10.4 B, median 10 B) — puts
   the round trip at 0.61×–0.67× up to 100k items and 0.86× at a
   million: a real lead, but not the 0.22× of a synthetic 4 KiB
   string. `sort_unstable` + `dedup` closes most of what is left
   (0.80×–0.87×) and wins outright at a million (0.65×), while staying
   deterministic.
4. **At the size real code deduplicates names, the choice is
   microseconds.** Deduplicating 1000 npm names — a lockfile's worth —
   took 32 µs through the set against 51 µs for the vector and 41 µs
   for `sort_unstable` + `dedup`. The set's advantage is asymptotic,
   so the inputs at which it is worth an `#[expect]` are much larger
   than a manifest.
5. **The set is not the lighter container it looks like.**
   Deduplicating 1M values with 100 distinct ones, the set added
   18 MiB on top of the 8 MiB input vector, because
   `HashSet::from_iter` reserves on the iterator's size hint: the
   table is sized for the input, duplicates included, not for the
   result. The O(unique) footprint the set is reached for appears only
   when the source has no usable size hint — the same workload behind
   a `filter` peaked at a few KiB for the set against the vector's
   8 MiB.

Names, at the lengths and sizes they actually occur in — npm names
sampled from the full registry list, crate names from the download
ranking, `total` items drawn from `distinct` of them:

| Pool (mean length)   | total → distinct | `HashSet` | `HashSet` + `sort` | `BTreeSet` | `sort` + `dedup` | `sort_unstable` + `dedup` |
|----------------------|------------------|-----------|--------------------|------------|------------------|---------------------------|
| npm names (20 B)     | 1k → 1k          | 0.62×     | 1.35×              | 1.23×      | 1.00× (51 µs)    | 0.80×                     |
| npm names (20 B)     | 10k → 10k        | 0.61×     | 1.16×              | 1.18×      | 1.00× (834 µs)   | 0.87×                     |
| npm names (20 B)     | 100k → 100k      | 0.63×     | 1.21×              | 1.18×      | 1.00× (12.8 ms)  | 0.82×                     |
| npm names (20 B)     | 1M → 100k        | 0.86×     | 0.84×              | 1.26×      | 1.00× (348 ms)   | 0.65×                     |
| crate names (10.4 B) | 1k → 1k          | 0.67×     | 1.44×              | 1.21×      | 1.00× (46 µs)    | 0.87×                     |

And at the sizes most code deduplicates a name list at all — a
package's dependencies, a workspace's members, a command's arguments:

| total → distinct | `HashSet` | `HashSet` + `sort` | `BTreeSet` | `sort` + `dedup` | `sort_unstable` + `dedup` |
|------------------|-----------|--------------------|------------|------------------|---------------------------|
| 10 → 10          | 1.63×     | 1.93×              | 1.23×      | 1.00× (0.34 µs)  | 1.00×                     |
| 25 → 25          | 1.19×     | 1.63×              | 1.22×      | 1.00× (1.07 µs)  | 0.98×                     |
| 50 → 50          | 1.25×     | 1.63×              | 1.17×      | 1.00× (2.04 µs)  | 0.82×                     |
| 100 → 100        | 1.30×     | 1.94×              | 1.50×      | 1.00× (3.74 µs)  | 0.90×                     |
| 200 → 200        | 0.90×     | 1.37×              | 1.16×      | 1.00× (10.4 µs)  | 0.75×                     |
| 1000 → 1000      | 0.73×     | 1.28×              | 1.12×      | 1.00× (66.8 µs)  | 0.73×                     |

Below ~200 items the round trip is the *slower* option outright —
1.2×–1.6× — because it pays an allocation and a hash per element where
the vector sorts in place.

Those tables are all `String`, though, and a string is the element
that flatters the set most: every comparison follows a pointer. Across
element types, each cell below is the round trip divided by the
`sort_unstable` + `dedup` the rule suggests, so above 1.00× the
flagged code is the slower one:

| Element (size)                    |    10 |   200 | 1 000 | 100 000 |
|-----------------------------------|-------|-------|-------|---------|
| `u32` (4 B)                       | 4.76× | 3.04× | 2.71× | 1.32×   |
| `u64` (8 B)                       | 5.58× | 3.12× | 2.64× | 1.51×   |
| `u128` (16 B)                     | 6.53× | 3.31× | 2.82× | 1.85×   |
| `Id(u64)` newtype (8 B)           | 5.27× | 2.92× | 2.51× | 1.33×   |
| `struct Pair`, derived (8 B)      | 4.55× | 2.40× | 2.10× | 1.12×   |
| `enum Kind`, derived (16 B)       | 4.14× | 2.28× | 2.20× | 1.05×   |
| `struct Keyed`, forwarding (32 B) | 4.35× | 2.17× | 1.72× | 0.95×   |
| `[u8; 32]` digest (32 B)          | 3.12× | 1.37× | 0.97× | 0.63×   |
| `Name(String)` newtype (24 B)     | 1.65× | 1.23× | 0.99× | 0.68×   |

Read down the first column: on ten elements the round trip costs four
to six times what the suggestion costs, for every element but a
string. Read along the rows: for the primitives, the newtype over one,
the derived struct and the derived enum it never becomes the faster
option at any size measured — 1.05× to 1.85× even at a hundred
thousand. The set's advantage belongs to elements whose comparison is
far dearer than their hash — one reached through a pointer (`String`,
and a newtype over one) or wide inline bytes (a 32-byte digest) — and
even there it arrives only past ~1000 elements.

A wrapper costs nothing either way: `Id(u64)` — the shape a
`derive_more::From` / `Display` newtype has, since the comparison
traits are always the std derives — tracks raw `u64` to within noise
at every size, as `Name(String)` tracks `String`. Hand-written
`Eq` / `Ord` that forward to one key field (`Keyed`, with a payload
riding along) behave like the primitive they forward to, not like the
32 bytes they carry. The element that decides this is the one the
comparison actually reads.

The `BTreeSet` column ran from 1.26× to 2.79× of the suggestion
across every element type and size here — slower in every row, which
is what the earlier tables found too.

### How these were measured

Dependency-free single-file programs, `rustc -O --edition 2021
-C target-cpu=native`, rustc 1.99.0-nightly, x86-64. Reproducing them
needs only this description; they are scratch files and are not
committed.

- **Subjects.** The five forms in the tables, each taking the source
  vector by value and returning the deduplicated one, so the output's
  allocation and drop are inside the timed region and the input's
  clone is outside it. `black_box` wraps the input and the result.
- **Timing.** One call per measurement above ~100k elements, best of
  five to nine. Below that a call is nanoseconds, so a batch of
  pre-cloned inputs is timed as a whole and divided, best of seven to
  nine batches.
- **Allocation.** A counting `GlobalAlloc` tracks live bytes and a
  high-water mark; the figure quoted is the peak above the live bytes
  at the moment the call started, so the input vector is excluded and
  the set, the scratch buffer and the output are not.
- **Workloads.** `total` items drawn with a xorshift from `distinct`
  values and shuffled, so the duplicate ratio is a parameter rather
  than an accident. Names come from two pools: the 4.44M-name npm
  registry list, and the 1000 most-downloaded crates.io names.
- **Element types.** Nine, chosen to separate what the comparison
  reads from what the value carries: `u32` / `u64` / `u128`; `[u8; 32]`
  as a digest; a newtype over `u64` with the std comparison derives
  and a `From` impl; a two-field struct and a three-variant enum, both
  derived; a struct whose hand-written `Eq` / `Ord` / `Hash` forward
  to one `u64` key while a 24-byte payload rides along; and a newtype
  over `String`.

## When the set is the right tool

Where a bullet below reaches for
`#[expect(perfectionist::temporary_dedup_set)]`, the rule does fire and
the suppression is the answer; the rest it never reaches at all.
`#[expect]` rather than `#[allow]` throughout, because
`perfectionist::allow_attributes` resolves an `#[allow]` of a
deterministically-firing lint into one.

- **The element is not `Ord`.** `sort_unstable` does not compile for
  it, so the rule does not fire; the gate is a trait-resolution check,
  not a heuristic. What is missing is a fix the *rule* can write, not
  a fix at all — see
  [When the element is not `Ord`](#when-the-element-is-not-ord) for
  the one a reader writes by hand. What it excludes
  is narrower than it sounds. std has almost nothing in this
  population — `Range`, `RangeInclusive`, `Discriminant`, `Layout`,
  `ThreadId`, `FileType`, while `io::ErrorKind` and `TypeId` are both
  `Ord` — so the gate is about domain types. pnpm's workspace has 27
  of those, and they are tag enums whose order would be declaration
  order, composite lookup keys, and cache keys: types reached with
  `contains`, not collected into vectors. None of the round trips in
  that workspace has one as its element, which is not a coincidence —
  a type gets `Ord` when somebody needed to order it, and the code
  that wants a deterministic list is the code that would have added
  the derive.
- **The set is read as a set.** A `contains` call, a `len`, an
  `insert` after the fact, a return, a store into a field, a borrow
  that outlives the expression — any of these and the value is not
  temporary, whatever else happens to it.
- **Deduplication preserves the input order.** The
  `seen.insert(item)`-in-a-`retain` idiom, `itertools::unique()`, and
  `indexmap::IndexSet` all keep first-occurrence order, which sorting
  destroys. They are not this shape — the set is consumed by `insert`
  calls, or the container is order-preserving — and `IndexSet` is not
  in the recognised set-type list for exactly this reason.
- **The walk is not re-collected.** `for name in
  names.into_iter().collect::<HashSet<_>>()` visits unique elements in
  an arbitrary order and never builds a sequence; there is nothing to
  sort and nothing to dedup, so the set is doing a job no vector does
  more cheaply.
- **Thousands of pointer-chasing elements whose order is genuinely
  unused.** The set leads only where a comparison costs far more than
  a hash — a `String` or `PathBuf`, a newtype over one, a wide digest
  — and only from about 10³ elements up, by about 1.5×. For a
  primitive, a newtype over one, a derived struct or enum, or an impl
  that forwards to one key field, there is no such band at all: the
  round trip measured slower at every size, by four to six times on
  ten elements. The honest remedy even inside the band
  is usually not to sort at all but to stop discarding the set: keep
  it, name it, and let the code that consumes it say it wants a set.
  Where a vector really is what the caller needs, `#[expect]` it on the
  strength of a measurement — a measurement, because the lead
  disappears the moment anything downstream wants a deterministic
  order.
- **The source is lazy and duplicates dominate.** Deduplicating a
  streamed million lines down to a few hundred holds a few hundred in
  a set against a million in a vector. `#[expect]` it, and note that
  the advantage disappears the moment the source is a `Vec` or any
  other sized iterator.

### When the element is not `Ord`

The gate stops the rule, not the reader. An element that enters a
`HashSet` carries `Eq + Hash` by construction, so the only piece
missing is an order — and an order its author can state is enough:

```rust
#[derive(PartialEq, Eq, Hash)]
struct Key {
    scope: Option<String>,
    bare: String,
}

// What the rule cannot rewrite.
let unique: Vec<Key> = keys.into_iter().collect::<HashSet<_>>().into_iter().collect();

// The same elements, by hand.
let mut unique: Vec<Key> = keys.into_iter().collect();
unique.sort_unstable_by(|a, b| {
    (a.scope.as_deref(), a.bare.as_str()).cmp(&(b.scope.as_deref(), b.bare.as_str()))
});
unique.dedup();
```

Each part of that shape is a place the obvious spelling fails:

- **`dedup` stays plain.** `Eq` is guaranteed — the set required it —
  so once the comparator has made equal elements adjacent, `dedup`
  removes exactly what the set removed. `dedup_by` is for a projection
  deliberately coarser than `==`, and its result is no longer the
  set's.
- **Not `sort_by_key`.** Its `K` is fixed independently of each `&T`,
  so a key borrowed from the element does not compile — one field or
  several, `|k| k.bare.as_str()` and `|k| (k.scope.as_deref(),
  k.bare.as_str())` alike. The keys that do compile are `Copy`
  projections, and a type whose ordering fields are all `Copy` would
  have derived `Ord` rather than reaching for a projection at all. So
  the advice is self-defeating exactly where it is needed: a
  borrow-check error, or a clone per key — the allocation the rewrite
  exists to remove. `sort_by` takes both elements and returns an
  `Ordering`, so nothing borrowed escapes and the projection compiles.
  `dedup_by_key` fails the same way, for the same reason.
- **`sort_by_cached_key` for an order that really is a materialised
  form** — a rendered name, a normalised string. It builds that form
  once per element where `sort_by_key` rebuilds it once per
  comparison.

The chained `style` spells the same thing `into_sorted_unstable_by`
with `into_deduped`.

That comparator is also why the rule stays silent here rather than
diagnosing: which fields, in which order, and whether `None` sorts
first are judgements about the type, and a wrong guess still compiles
while changing the output.

## What to lint

`LateLintPass`. Type resolution decides every part of the trigger: the
container types, the `Ord` bound, and whether the walk lands in a
sequence.

A **set round trip** has these parts, all of which must hold:

1. **The build.** A `collect::<S>()` (or `S::from_iter(..)`) whose
   receiver implements `Iterator`, where `S` is `HashSet<T, _>` or
   `BTreeSet<T>` — any hasher, so an alias that only swaps it
   (`rustc_hash::FxHashSet`) is covered without configuration — or a
   type named in `extra_set_types`. A set that merely *arrives* (a
   parameter, a field, a function's return) is not a build: the
   duplicates were dropped by whoever built it, so the walk is a
   one-way conversion between containers and there is nothing for
   `sort_unstable` + `dedup` to replace.
2. **The walk.** The set is consumed exactly once, by `into_iter()`,
   `iter()`, `iter().cloned()`, `iter().copied()`, or `drain(..)`, and
   is used for nothing else.
3. **The landing.** The walk, after any number of adapters, is
   collected into a sequence: `Vec<U>`, `VecDeque<U>`, or `Box<[U]>`,
   including the fallible forms a `collect` produces from an iterator
   of `Result` / `Option` (`Result<Vec<U>, E>`). Landing in a map or a
   set is not this shape: `keys().chain(...).collect::<BTreeSet<_>>()
   .into_iter().filter_map(...).collect::<BTreeMap<_, _>>()` ends
   somewhere a sort cannot replace.

Plus one gate: **`T: Ord`**, resolved against the element type. Without
it the suggestion does not compile, so the rule must not fire.

One discovery locus per form in [Statement](#statement):

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
unique.sort_unstable();
unique.dedup();

let mut names: Vec<&str> = entries.iter().map(Entry::name).collect();
names.sort_unstable();
names.dedup();
let names: Vec<String> = names.into_iter().map(str::to_owned).collect();
```

**Left alone:**

```rust
// The set is read as a set.
let known: HashSet<&str> = manifest.dependencies.keys().copied().collect();
requested.retain(|name| known.contains(name));

// The set arrives; this is a conversion, not a round trip.
fn sorted_names(names: HashSet<String>) -> Vec<String> {
    let mut names: Vec<String> = names.into_iter().collect();
    names.sort_unstable();
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

## Examples from a consumer's workspace

pnpm's Rust workspace — 109 crates, with a `dylint.toml` that already
pins this plugin — carries three round trips, against 29
`dedup` call sites that already write the vector form (27 of them
directly after a sort). Quoted at
[`f607801`](https://github.com/pnpm/pnpm/commit/f60780170c962d938082562d26fdbd4689c26a85),
so each stays a citation rather than a copy that drifts.

### The round trip that sorts afterwards anyway

```rust
/// Deduplicate and sort `names` by code unit, matching pnpm's
/// `sortUniqueStrings` (a `Set` then `lexCompare`).
fn sort_unique(names: Vec<String>) -> Vec<String> {
    let mut unique: Vec<String> = names
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    unique.sort();
    unique
}
```

The set contributes nothing here at all: the arbitrary order it
produced is overwritten on the next line, and the duplicates it
dropped are the ones `dedup` drops. The doc comment names the
JavaScript function the code was ported from, which is where the shape
comes from — `[...new Set(xs)].sort()` is the right idiom in a
language whose `Set` keeps insertion order and whose arrays have no
`dedup`. The whole function is:

```rust
fn sort_unique(mut names: Vec<String>) -> Vec<String> {
    names.sort_unstable();
    names.dedup();
    names
}
```

### A set between a `map` and a fallible landing

```rust
let snapshots = paths
    .into_iter()
    .collect::<BTreeSet<_>>()
    .into_iter()
    .map(MetadataFile::capture)
    .collect::<Result<Vec<_>>>()?;
```

`paths` is the function's own `Vec<PathBuf>` parameter, so the rewrite
takes it `mut` and deduplicates it where it stands, before the map
that was reading out of the set:

```rust
paths.sort_unstable();
paths.dedup();
let snapshots =
    paths.into_iter().map(MetadataFile::capture).collect::<Result<Vec<_>>>()?;
```

### A set whose whole job is the `Vec` it becomes

```rust
/// The git sources the locked packages come from, deduplicated so the
/// managed Cargo configuration declares each one once.
pub(super) fn git_sources(&self) -> Vec<GitSource> {
    self.git
        .iter()
        .map(|package| GitSource::clone(&package.source))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
```

The rewrite collects the `Vec` the signature already promises, then
sorts and dedups it. `GitSource` is `Ord` because the `BTreeSet`
required it — which is true of every `BTreeSet` instance, so the
element gate can only ever exclude a `HashSet` one.

### Left alone in the same workspace

These are the more useful half of the sample: each fails a different
clause of the trigger, and a rule that reports any of them is wrong.

```rust
// Built by a helper, then filled through `&mut`: the build is not a
// `collect`, and the set is used twice.
let mut out: BTreeSet<String> = collect_own_files(pkg_dir, manifest, options.workspace_dir)?;
collect_bundled_files(pkg_dir, manifest, &mut out)?;
Ok(out.into_iter().collect())

// Built empty and filled by a loop: the build is not a `collect`.
let mut rel_dirs: HashSet<&str> = HashSet::new();
for entry in cas_paths.keys() {
    // ...
    rel_dirs.insert(rel);
}

// Read as a set — `absorb_compatible` removes from it — before the
// walk that looks like the anti-pattern.
let mut unresolved: HashSet<DepPath> = dep_paths.iter().cloned().collect();
while let Some(largest) = current.pop() {
    absorb_compatible(graph, &largest, &mut current, &mut dep_paths_map, &mut unresolved);
    current.sort_by(dep_count_sorter);
}
if !unresolved.is_empty() {
    let mut leftover: Vec<DepPath> = unresolved.into_iter().collect();
    leftover.sort();
    remaining_duplicates.push(leftover);
}

// The landing is a comparison: two sets compared to ignore order.
assert_eq!(
    carried.all_peer_dep_names.iter().collect::<BTreeSet<_>>(),
    from_scratch.all_peer_dep_names.iter().collect::<BTreeSet<_>>(),
);

// The landing is a length: a duplicate check, not a sequence.
deleted.iter().collect::<HashSet<_>>().len() == deleted.len()
```

### The scale this code runs at

The data the three sites deduplicate is small — package names awaiting
build approval, metadata paths, git sources. That is the normal scale
for this kind of code: across pnpm's own workspace a package declares
a median of 1 and a p90 of 17 direct dependencies, its lockfile
carries 218 workspace importers and 1700 packages, and the largest
single manifest declares 110. Every one of those numbers sits in the
range where the vector form is also the faster one.

Everything in this section is contributor-facing. A shipped doc — the
`declare_tool_lint!` rustdoc and the catalogue generated from it — may
not name the project a rule was distilled from; see
[`CLAUDE.md`](../CLAUDE.md#shipped-docs-address-the-consumer-not-the-contributor).

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
# target. Defaults to `false`, so a test is held to the same shape as
# the code it exercises — a test fixture is read as an example of how
# the project writes Rust, and the sizes it runs at are the sizes
# where the vector form is also the faster one.
exempt_tests = false
```

The *form* of the suggestion — the statements, or the
`into_sorted_unstable().into_deduped()` chain from
[Statement](#statement) — is a `style` knob whose values are
`statements` (the std-only form, the default) and `chained` (which
requires the consumer to depend on both crates). Ship it with the
second rewrite, not before: a one-variant `style` enum carries no
information, which is the call `perfectionist::overly_long_print_macro`
already made for its own pending rewrite.

## Implementation notes

- **Autofix, chained form.** Fire the machine-applicable rewrite only
  where the round trip is a `let` initializer, which is where it
  occurs in practice: replace the initializer with the collect that
  produced the set's input, then insert `sort_unstable` and `dedup`
  statements after it, adding `mut` to the binding if it lacks one. In
  any other expression position the rewrite needs a block, so emit the
  help text without a suggestion.
- **Autofix, bound form.** Delete the `let` that builds the set,
  re-point the second binding's initializer at the first's, and append
  the two statements. The use scan has already proved the name is dead
  after the walk.
- **Suggest `sort_unstable`, not `sort`.** Stability decides one
  thing: the relative order of elements that compare `Equal`. This
  rewrite has none to preserve — the value it replaces came out of a
  `HashSet`, whose order is arbitrary, or out of a `BTreeSet`, which
  had already collapsed `Equal` elements to one. So the stable sort
  guarantees something about an input order that the flagged code
  never had, and charges for it: `sort_unstable` measured faster in
  every row of every table above, including already-sorted input
  (0.55× at a thousand names, 0.96× at a hundred thousand), and
  allocates nothing where the stable sort allocates scratch. It also
  makes `clippy::stable_sort_primitive` a non-issue, since that lint
  asks for exactly this. The one measured exception is an element
  whose comparison is drastically more expensive than its hash — 4 KiB
  strings sharing a 4100-byte prefix, where the stable sort's lower
  comparison count put it at 0.95× of `sort_unstable` — and an element
  that expensive belongs in the `#[expect]` case above, not in a
  different suggestion.
- **Test-code exemption.** `exempt_tests` reaches the shared helpers
  per
  [Recognising test-exclusive code](./IMPLEMENTATION_CONVENTIONS.md#recognising-test-exclusive-code),
  rather than matching `cfg(test)` itself.
- **The comparator family is help text, not autofix.**
  `sort_unstable` and `dedup` are the one pair that reproduces a set's
  semantics knowing nothing about the element beyond `T: Ord`: the
  sort orders by the element's own `Ord`, which is what a `BTreeSet`
  used, and `dedup` removes by `PartialEq`, which is what `Eq` gave a
  `HashSet`. A comparator variant needs an order the rule would have
  to invent, and an invented one changes *which* duplicates survive,
  so the suggestion stays that pair and the diagnostic points the
  reader at the by-hand route instead —
  [When the element is not `Ord`](#when-the-element-is-not-ord) is
  where that route, and the spellings of it that do not compile, are
  written down.
- **Dropping a needless clone.** Where the walk is `iter().cloned()`
  or `iter().copied()`, the rewrite drops it: the set was about to be
  dropped, so the elements can be moved. Say so in the diagnostic
  rather than silently changing the chain.
- **Applicability, and what gates it.** Both branches produce the same
  multiset under the trait contracts the rule already assumes, so
  neither is ever *wrong*; what differs is how much room the
  measurement leaves for a human to disagree.

  `MachineApplicable` for the `BTreeSet` branch. Its output is the
  vector the rewrite produces, element for element and order for
  order, and it measured slower than the suggestion in every row of
  every table (1.26×–2.79× on the element-type sweep).

  `MachineApplicable` for the `HashSet` branch too, but only where
  the element makes the answer decisive: **no indirection, and a
  layout of at most 16 bytes** (`cx.layout_of`, with a reference, a
  raw pointer or a heap-owning field disqualifying). That is the set
  of elements the sweep found no size at which the round trip wins —
  4.1×–6.5× slower on ten elements, still 1.05×–1.85× slower on a
  hundred thousand — and it covers the primitives, the newtypes over
  them, and the derived structs and enums. A `HashSet`'s iteration
  order is unspecified, so no correct program can depend on the order
  being replaced, which is what makes the rewrite safe to apply
  unattended here.

  `MaybeIncorrect` for every other `HashSet` element — a `String`, a
  `PathBuf`, a newtype over one, a 32-byte digest. The enum name
  undersells it: the suggestion is valid Rust and correct, but past
  ~1000 elements it can cost about 1.5×, and a trade-off is a call for
  a human rather than for `cargo fix`. The 16-byte line is deliberately
  conservative at one edge: a wide struct whose comparison forwards to
  one small key behaves like that key, not like its own size, and is
  held back anyway because a `cmp` impl can read whatever it likes.

### Difficulty

**Medium.** The chained form is a spine walk plus the container-type
checks and a trait-bound query, all of which the late pass has to
hand. The
bound form adds a body-local use scan, and that scan is where a wrong
implementation false-positives: a `contains` call, a borrow that
escapes into a closure, or a second walk all have to disqualify the
set. A conservative first cut ships the chained form alone, which
needs no scan at all, and adds the bound form once the disqualifying
uses have fixtures.

## Default state

Active by default. The trigger is narrow, the `Ord` gate removes the
one case with no fix, and the remaining exceptions are performance
trade-offs a crate states once with `#[expect]`. A crate
that deduplicates large string collections whose order nothing reads
— where the measurement favours the set — is the crate that turns the
rule off in `[perfectionist].disable`.

## Interaction with clippy and sibling rules

- **`clippy::needless_collect` does not cover this.** It matches
  `Vec`, `VecDeque`, `LinkedList`, and `BinaryHeap` only — dropping a
  set's `collect` would change which elements survive, so it stays out
  of scope there. This rule keeps the deduplication and changes how it
  is spelled, which is why it can cover the types Clippy's lint
  cannot.
- **`clippy::stable_sort_primitive`** wants `sort_unstable` wherever
  stability cannot matter, which is what this rule's suggestion emits,
  so the two never disagree. `clippy::derive_ord_xor_partial_ord` and
  `clippy::derived_hash_with_manual_eq` police the `Ord`/`Eq`
  agreement that the `BTreeSet` branch's rewrite assumes.
- **`perfectionist::overly_long_method_chain`** measures length; this
  measures shape. A two-line round trip is short and still flagged
  here; a nine-call chain with no round trip is flagged there and not
  here.
- **No general round-trip rule stands behind this one.** A round trip
  through a `Vec`, or through a set whose element is not `Ord`, is
  reported by nothing: the rule that would have covered those was
  closed (see the source above), so an element that fails the `Ord`
  gate is simply left alone rather than handed on.
- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
