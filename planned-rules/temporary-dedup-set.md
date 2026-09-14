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
let mut unique: Vec<PackageId> =
    ids.into_iter().collect::<HashSet<_>>().into_iter().collect();

// Avoid: the same round trip with the set named.
let deduplicated: HashSet<PackageId> = ids.into_iter().collect();
let mut unique: Vec<PackageId> = deduplicated.into_iter().collect();

// Prefer:
let mut unique: Vec<PackageId> = ids.into_iter().collect();
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

let unique = ids
    .into_iter()
    .collect::<Vec<PackageId>>()
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
computes the right set of values, and on a large enough input whose
elements are expensive to compare and whose resulting order nothing
reads, it is the faster of the two — see
[When the set is the right tool](#when-the-set-is-the-right-tool),
which is the part of this file that decides whether the rule is worth
enabling in a given crate. For most element types, though, the
preference is free at every size measured: the vector form is also the
quicker one.

The anti-pattern is the round trip, not the set. A `HashSet` used as
a `HashSet` — membership tested, the value kept, passed on, returned —
is the right container for what it does, and the rule never reaches
it; "keep the set" is the first remedy the diagnostic offers, not a
concession. What it flags is a set built only to be walked straight
back into a linear sequence, which is the one shape in which both of
the set's properties go unused: its membership test is never called
and its order is discarded on arrival.

The case against that shape is not the clock, and the two are worth
keeping apart. A set that exists for one expression asks for
deduplication by building a container whose whole contract — members,
no order — is thrown away on the next line; where that is the wrong
tool it is the wrong tool whether or not the rewrite would run
faster. What the measurements decide is what the rule may *suggest*,
not what it may flag.

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

Every cell is that subject's time as a multiple of `sort_unstable` +
`dedup`, the pair the rule suggests, so 1.00× is the code the rule
would have written and anything above it is slower than that.

| Workload (input → unique)                  | `HashSet` | `HashSet` + `sort` | `BTreeSet` | `sort` + `dedup` | `sort_unstable` + `dedup` |
|--------------------------------------------|-----------|--------------------|------------|------------------|---------------------------|
| 1M `u64`, all distinct                     | 2.87×     | 4.35×              | 1.83×      | 1.39×            | 1.00× (18.7 ms)           |
| 1M `u64`, 1% distinct                      | 2.92×     | 2.94×              | 1.31×      | 1.31×            | 1.00× (11.1 ms)           |
| 500k `String` (16 B), all distinct         | 0.69×     | 2.54×              | 1.94×      | 1.65×            | 1.00× (77 ms)             |
| 500k `String` (216 B, 200 B shared prefix) | 0.29×     | 1.71×              | 1.30×      | 1.28×            | 1.00× (308 ms)            |
| 200k `String` (4 KiB, shared prefix)       | 0.23×     | 1.20×              | 1.00×      | 0.98×            | 1.00× (1034 ms)           |
| 500k `String` (216 B, distinct prefix)     | 0.24×     | 2.04×              | 1.42×      | 1.42×            | 1.00× (331 ms)            |
| 200k `String` (4 KiB, distinct prefix)     | 0.97×     | 2.43×              | 1.18×      | 1.13×            | 1.00× (278 ms)            |

The two distinct-prefix rows separate two variables the others
confound, and the separation is worth reading carefully, because each
variable pushes a different way.

Hold the length and move the *difference* to the front. At 4 KiB the
round trip falls from 0.23× to parity: every comparison had been
walking 4100 shared bytes and now stops at the first, while the set
still hashes all 4116 of every element. At 216 B the same swap barely
registers, 0.29× to 0.24×, because at half a million elements the
dereference in front of that comparison is a cache miss into 108 MB,
and 200 sequential bytes behind a cache miss are free. The isolation
rows below show the same 200-odd bytes costing 16% once the collection
is small enough that nothing misses.

Now hold the prefixes distinct and move the length. From 16 B to
216 B the round trip goes the other way, 0.69× to 0.24×, because the
sort's pointer chase now ranges over 108 MB of heap where it ranged
over 8, while hashing still streams.

So length cuts both ways and the scale decides which: it costs the
sort cache misses across a larger heap, and it costs the set a hash
proportional to every byte. Below a few hundred bytes the first
dominates and the set pulls ahead; by 4 KiB the second has caught up.
A shared prefix only ever costs the sort. Both of those are
half-a-million-element conclusions, and the real pools below run at a
thousand, where the heap fits in cache and the length half inverts.
(The 4 KiB workloads hold 800 MB and vary by about a tenth run to
run; their figures are medians of three.)

The findings that shape the rule:

1. **The `BTreeSet` round trip never won.** It ran between 1.00× and
   1.94× here and between 1.34× and 3.85× across the element sweep
   below — never faster than the suggestion in any row of any table —
   allocated more (9.7 MiB against nothing on the 1M-`u64` case), and
   produced the identical sorted vector. There is no workload in which
   to prefer it, which is why the `BTreeSet` branch is the
   machine-applicable one. "Identical" rests on `Ord` agreeing with
   `Eq`, which the `Ord` trait requires: a `BTreeSet` decides
   duplicates by `cmp`, `dedup` decides them by `==`, so a type whose
   `cmp` returns `Equal` for values that are not `==` would keep more
   elements after the rewrite than before.
   `clippy::derive_ord_xor_partial_ord` and
   `clippy::derived_hash_with_manual_eq` police that contract; this
   rule assumes it, and says so rather than re-deriving it.
2. **The `HashSet` round trip wins on elements that are expensive to
   compare — which is fewer types than it sounds, and not simply the
   long ones — and only while its order goes unused.** The set spends
   one hash per element where the sort spends log n comparisons, so it
   leads once a comparison stops being far cheaper than a hash. Hence
   0.23×–0.69× on the string rows where the set leads, and 0.97× on
   the 4 KiB row whose prefixes diverge — against 1.20×–2.54× for the
   same code once a `sort` is appended to make the output
   deterministic, which is the comparison to make as soon as anything
   observes the order. For `u64`, where a comparison is a register
   instruction, the round trip is ~2.9× slower.
3. **The synthetic strings above overstate it; real pools are cheaper
   to sort.** Repeating the measurement on four real pools — npm
   package names, the crates.io download ranking, pnpm's lockfile keys
   and its repository's file paths — puts a thousand-item round trip
   at 0.73×–0.80×, and a million npm names at 1.09×: a real lead, but
   not the 0.23× of a synthetic 4 KiB string. None of the four carries
   enough shared prefix to reproduce what the synthetic rows isolate,
   which the tables below take up.
4. **At the size real code deduplicates names, the choice is
   microseconds.** A thousand npm names — a lockfile's worth — took
   31 µs through the set against 41 µs for the suggestion. The set's
   advantage is asymptotic, so the inputs at which it is worth an
   `#[expect]` are much larger than a manifest.
5. **The set is not the lighter container it looks like.**
   Deduplicating 1M values with 100 distinct ones, the set added
   18 MiB on top of the 8 MiB input vector, because
   `HashSet::from_iter` reserves on the iterator's size hint: the
   table is sized for the input, duplicates included, not for the
   result. The O(unique) footprint the set is reached for appears only
   when the source has no usable size hint — the same workload behind
   a `filter` peaked at a few KiB for the set against the vector's
   8 MiB. `sort_unstable` allocates nothing at all; the stable sort is
   what needs the scratch buffer.

Real strings next, at the lengths and sizes they occur in. The pools
are a sample of the npm registry's name list, the crates.io download
ranking, every `name@version(peers)` key in pnpm's own lockfile, and
every file path in its repository; `total` items are drawn from
`distinct` of them.

Two shared-prefix figures come with them, because only one is what a
sort pays:

| Pool                  | entries | mean length | prefix: neighbour | prefix: random pair |
|-----------------------|---------|-------------|-------------------|---------------------|
| crates.io top names   | 1 000   | 10.4 B      | 4.3 B             | 0.09 B              |
| npm package names     | 300 000 | 20.0 B      | 7.8 B             | 0.18 B              |
| pnpm lockfile keys    | 2 181   | 26.4 B      | 11.0 B            | 0.48 B              |
| pnpm repo file paths  | 7 252   | 57.4 B      | 41.3 B            | 10.30 B             |

The neighbour figure is how much an entry shares with the one beside
it in sorted order, and it is the one that comes to hand: pnpm's paths
share 41 of their 57 bytes, so a comparison sounds expensive. But a
comparison sort spends most of its comparisons on pairs that are far
apart, and shared prefixes in real data are local — one directory, one
`@types/` scope. Drawn at random from the same pool, two paths share
10 bytes and two package names under half of one. The neighbour figure
overstates what the sort walks by 40× to 80×, so everything below
quotes the random-pair figure.

| Pool (mean length)   | total → distinct | `HashSet` | `HashSet` + `sort` | `BTreeSet` | `sort` + `dedup` | `sort_unstable` + `dedup` |
|----------------------|------------------|-----------|--------------------|------------|------------------|---------------------------|
| npm names (20 B)     | 1k → 1k          | 0.77×     | 1.67×              | 1.42×      | 1.15×            | 1.00× (41 µs)             |
| npm names (20 B)     | 10k → 10k        | 0.69×     | 1.32×              | 1.32×      | 1.16×            | 1.00× (726 µs)            |
| npm names (20 B)     | 100k → 100k      | 0.68×     | 1.39×              | 1.45×      | 1.22×            | 1.00× (10.3 ms)           |
| npm names (20 B)     | 1M → 100k        | 1.09×     | 1.24×              | 2.07×      | 1.58×            | 1.00× (217 ms)            |
| crate names (10.4 B) | 1k → 1k          | 0.73×     | 1.65×              | 1.39×      | 1.13×            | 1.00× (41 µs)             |
| lockfile keys (26 B) | 1k → 1k          | 0.80×     | 1.45×              | 1.41×      | 1.17×            | 1.00× (66 µs)             |
| lockfile keys (26 B) | 10k → 1k         | 0.75×     | 0.84×              | 1.37×      | 1.18×            | 1.00× (932 µs)            |
| file paths (57 B)    | 1k → 1k          | 0.80×     | 1.43×              | 1.36×      | 1.15×            | 1.00× (79 µs)             |
| file paths (57 B)    | 10k → 1k         | 0.76×     | 0.83×              | 1.27×      | 1.11×            | 1.00× (1.2 ms)            |

At a thousand items the four pools land between 0.73× and 0.80×
against the 0.23×–0.97× the synthetic rows span. To find out why the
band is so flat, hold a thousand `String`s and move one variable at a
time — absolute times here, because the question is which side of the
comparison moves:

| 1 000 × `String` | random-pair prefix | `HashSet` | `sort_unstable` + `dedup` | ratio |
|------------------|--------------------|-----------|---------------------------|-------|
| 20 B             | 0.03 B             | 39.7 µs   | 63.8 µs                   | 0.62× |
| 20 B             | 16.03 B            | 39.8 µs   | 63.4 µs                   | 0.63× |
| 57 B             | 0.03 B             | 51.1 µs   | 62.6 µs                   | 0.82× |
| 57 B             | 41.03 B            | 51.5 µs   | 72.4 µs                   | 0.71× |

The two bill different parties. A shared prefix never touches the set
— 39.8 µs against 39.7, 51.5 against 51.1, because a hash reads every
byte wherever two entries diverge — and taxes the sort, but only once
there is enough of it: 16 bytes cost nothing (63.4 µs against 63.8),
41 bytes cost 16% (72.4 against 62.6). Content length does the
reverse. At this size it leaves the sort alone — 62.6 µs on 57-byte
entries against 63.8 on 20-byte ones, since comparisons still decide
in the first byte or two and a thousand entries stay in cache — and
taxes the set 29%, 39.7 µs to 51.1, because the hash reads all of it.

That accounts for the real pools. They carry length, which costs the
set, and no random-pair prefix to hand back, so the round trip's lead
narrows as they lengthen: 0.73× at 10 B to 0.80× at 57 B. pnpm's file
paths are the one pool with a prefix worth anything, and 10 bytes of
it puts them at 0.80×, between the 0.82× of none and the 0.71× of 41.
The 500k rows above invert the length half — past cache the sort's
pointer chase outgrows the hash — so neither table carries to the
other's scale.

One thing every pool still confounds: a `String`'s bytes live behind a
pointer, so both its width and its indirection are candidates for the
set's lead. Hold the content at twenty distinct bytes and move only
where those bytes live.

| 20 B values, all distinct | element    | `sort_unstable` + `dedup` | `HashSet` round trip | ratio |
|---------------------------|------------|---------------------------|----------------------|-------|
| 1 000                     | `[u8; 20]` | 48.6 µs                   | 29.0 µs              | 0.60× |
| 1 000                     | `String`   | 49.9 µs                   | 27.9 µs              | 0.56× |
| 100 000                   | `[u8; 20]` | 8.2 ms                    | 3.9 ms               | 0.48× |
| 100 000                   | `String`   | 10.0 ms                   | 4.6 ms               | 0.46× |
| 1 000 000                 | `[u8; 20]` | 97.0 ms                   | 140.4 ms             | 1.45× |
| 1 000 000                 | `String`   | 338.1 ms                  | 155.5 ms             | 0.46× |

At a thousand elements the pointer costs the sort 3%: the two sorts
are the same sort. Nothing at that scale is explained by chasing it,
and the only thing that moves the sort there is bytes it actually
walks, which is the shared prefix above. At a million the pointer is
the whole story — the same sort over the same twenty bytes takes 97 ms
inline and 338 ms behind a pointer, because each comparison has become
a miss into a heap no cache holds — and it is indirection rather than
width that decides the outcome: inline, the round trip *loses* at a
million (1.45×); behind a pointer it wins by exactly the margin it won
by at a hundred thousand (0.46×). A shared prefix there is noise,
97.8 ms against 97.0.

So the two mechanisms trade places. Below cache the sort pays for the
bytes it walks and the indirection is free; past cache it pays for the
indirection and the bytes are free. Only the second regime makes the
round trip defensible, which is why the applicability gate turns on
indirection and not on length.

The rows where duplicates dominate narrow the comparison that matters.
Appending the `sort` roughly doubles the round trip while every
element is distinct, because it sorts everything the set held; at ten
duplicates per entry it adds a tenth, because it sorts only what
survived. A round trip whose order is observed is dearest exactly when
it dedupes least.

And at the sizes most code deduplicates a name list at all — a
package's dependencies, a workspace's members, a command's arguments:

| total → distinct | `HashSet` | `HashSet` + `sort` | `BTreeSet` | `sort` + `dedup` | `sort_unstable` + `dedup` |
|------------------|-----------|--------------------|------------|------------------|---------------------------|
| 10 → 10          | 2.21×     | 2.87×              | 1.63×      | 0.93×            | 1.00× (0.18 µs)           |
| 25 → 25          | 1.35×     | 2.16×              | 1.48×      | 0.99×            | 1.00× (0.63 µs)           |
| 50 → 50          | 1.26×     | 1.98×              | 1.57×      | 1.30×            | 1.00× (1.38 µs)           |
| 100 → 100        | 1.19×     | 2.08×              | 1.48×      | 1.09×            | 1.00× (2.87 µs)           |
| 200 → 200        | 0.98×     | 1.80×              | 1.46×      | 1.24×            | 1.00× (6.67 µs)           |
| 1000 → 1000      | 0.79×     | 1.68×              | 1.41×      | 1.22×            | 1.00× (43.1 µs)           |

Below about two hundred items the round trip is the slower option
outright — 1.2× to 2.2× — because it pays an allocation and a hash per
element where the vector sorts in place.

Those tables are all `String`, though, and a string is the element
that flatters the set most: every comparison follows a pointer. Across
element types:

| Element (`size_of`)               |    10 |   200 | 1 000 | 100 000 |
|-----------------------------------|-------|-------|-------|---------|
| `u32` (4 B)                       | 5.59× | 3.18× | 2.54× | 1.40×   |
| `u64` (8 B)                       | 7.72× | 3.26× | 2.66× | 1.51×   |
| `u128` (16 B)                     | 8.74× | 3.35× | 2.79× | 1.79×   |
| `Id(u64)` newtype (8 B)           | 7.55× | 2.96× | 2.51× | 1.35×   |
| `struct Pair`, derived (8 B)      | 6.04× | 2.40× | 2.11× | 1.11×   |
| `enum Kind`, derived (16 B)       | 5.37× | 2.28× | 2.21× | 1.06×   |
| `struct Keyed`, forwarding (32 B) | 5.37× | 2.20× | 1.81× | 0.92×   |
| `[u8; 32]` digest (32 B)          | 3.31× | 1.28× | 1.00× | 0.65×   |
| `Name(String)` newtype (24 B)     | 2.08× | 1.02× | 0.81× | 0.63×   |

Read down the first column: on ten elements the round trip costs five
to nine times what the suggestion costs, for every element but a
string or a digest. Read along the rows: for the primitives, the
newtype over one, the derived struct and the derived enum it never
becomes the faster option at any size measured — 1.06× to 1.79× even
at a hundred thousand. The set's advantage belongs to elements whose
comparison is far dearer than their hash — one reached through a
pointer (`String`, and a newtype over one) or wide inline bytes (a
32-byte digest) — and even there it arrives only past a thousand
elements.

Two different measurements wear the same unit in these tables, and
only one of them moves the result. The element table's figure is
`size_of` — the inline width of a value as it sits in the vector, 24
bytes for a `String` whatever it holds. The pools' figure is
`str::len`, the heap bytes behind that pointer. The applicability gate
reads the first and never the second.

Nor does it need to, because the content length is not what a
comparison scales with. `str` orders as `as_bytes().cmp(..)`,
lexicographically, so a comparison stops at the first differing byte,
and two entries drawn at random from any of the name pools share under
half a byte, so the bytes past that point are never read — which is
why the sort takes the same 63 µs on 20-byte and 57-byte entries. What
is left is the sort's own bookkeeping; the dereference behind each
comparison costs 3% at this size, as the inline rows show. Content
length bills the set instead, through the hash. The
sort only starts paying for bytes where a prefix is shared across
distant pairs (41 bytes of it cost it 16%) or where the heap outgrows
cache, which is what the 216 B and 4 KiB rows are built to show —
0.29× and 0.23×, against 0.69× for distinct 16-byte strings.

Equality behaves differently again, and it is the set's tool rather
than the sort's. `HashSet` confirms a hash match with `==`, and slice
equality checks the two lengths before it touches a byte, so a
candidate of the wrong length is rejected outright. `dedup` uses `==`
too, once per adjacent pair. Only the sort uses `cmp`.

A wrapper costs nothing either way: `Id(u64)` — the shape a
`derive_more::From` / `Display` newtype has, since the comparison
traits are always the std derives — tracks raw `u64` to within noise
at every size, as `Name(String)` tracks `String`. Hand-written
`Eq` / `Ord` that forward to one key field (`Keyed`, with a payload
riding along) behave like the primitive they forward to, not like the
32 bytes they carry. The element that decides this is the one the
comparison actually reads.

## When the set is the right tool

Where a bullet below reaches for
`#[expect(perfectionist::temporary_dedup_set)]`, the rule does fire and
the suppression is the answer; the rest it never reaches at all.

- **The element is not `Ord`.** `sort_unstable` does not compile for
  it, so the rule has no suggestion to make; the gate is a
  trait-resolution check, not a heuristic. What is missing is a fix
  the *rule* can write rather than a fix at all — the section below
  has the one a reader writes by hand, and the setting that decides
  which of these the rule mentions anyway.
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
- **Thousands of elements whose comparison reads many bytes, whose
  order is genuinely unused.** The set leads only where a comparison
  costs far more than a hash — a `String` or `PathBuf`, a newtype over
  one, a wide digest — and only from about 10³ elements up, where it
  runs at 0.63×–0.81× of the suggestion. For a primitive, a newtype
  over one, a derived
  struct or enum, or an impl that forwards to one key field, there is
  no such band at all: the round trip measured slower at every size,
  by five to nine times on ten elements. The honest remedy even inside
  the band is usually not to sort at all but to stop discarding the
  set: keep it, name it, and let the code that consumes it say it
  wants a set.
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

What the gate excludes is narrower than it sounds. std has almost
nothing in this population — `Range`, `RangeInclusive`, `Discriminant`,
`Layout`, `ThreadId`, `FileType`, while `io::ErrorKind` and `TypeId`
are both `Ord` — so it is about domain types. pnpm's workspace has 27
of those, and they are tag enums whose order would be declaration
order, composite lookup keys, and cache keys: types reached with
`contains`, not collected into vectors. None of the round trips in
that workspace has one as its element, which is not a coincidence — a
type gets `Ord` when somebody needed to order it, and the code that
wants a deterministic list is the code that would have added the
derive.

Where one does turn up, the gate stops the rule, not the reader. An
element that enters a `HashSet` carries `Eq + Hash` by construction,
so the only piece missing is an order — and an order its author can
state is enough:

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
- **`sort_by_cached_key` only for an order that really is a
  materialised form**, and only as a fallback — see
  [Finding a view, cheaply](#finding-a-view-cheaply) for why a
  suggestion that allocates is a last resort rather than a choice.

The chained `style` spells the same thing `into_sorted_unstable_by`
with `into_deduped`.

That comparator is also why the rule stays silent here rather than
diagnosing: which fields, in which order, and whether `None` sorts
first are judgements about the type, and a wrong guess still compiles
while changing the output.

A narrower case sits inside this one: an element whose internals are
unreachable too — private fields, no accessor, no `Deref` — leaving no
*cheap* comparator to write. A rendered form does not rescue it.
`Display` allocates, so a type whose only view is its rendering
reaches the `sort_by_cached_key` fallback and never the comparator
route, which is to say it is in this case as far as the search is
concerned. Little else of it survives contact with the language. A
public enum is never in it, since its variants are its API and `match`
orders it. Privacy is
relative, so only a *downstream* crate can be stuck; the crate that
defines the type can always derive. And determinism was never the same
thing as ordering: deduplicating in place preserves the input's order,
which is deterministic whenever the input was, and needs only the
`PartialEq` the set already required. The remedy there is to stop
scrambling rather than to sort — and the rule is silent for a third
reason, that this rewrite is quadratic and whether that trade is right
depends on how many elements there are, which the rule cannot see.
Across pnpm's 109 crates one type is in it. `PkgVerPeer` exposes its
`version` through an accessor and keeps `prefix` and `peer` private,
so a downstream crate can compare part of it cheaply and the whole of
it only by rendering — and its `Display` renders all three at once.
One more type is opaque to its parent module alone, which the crate
that defines it can fix with a derive.

#### Finding a view, cheaply

Naming a projection in the help text means finding one, and the search
has to stay proportional to the element type rather than to the crate.
It runs once per type, memoised on its `DefId`, and never leaves that
type's own API:

- **The fields, where they are visible at the violation.** Every field
  accessible from the module being linted — private within the same
  module, `pub(crate)` within the crate, `pub` from anywhere — with
  each field's type either `Ord` or recursively in this same state,
  under a small depth cap. A visible enum is always here: its variants
  are its API, so `match` orders it.
- **A fixed list of view traits**, each a single impl query, each
  therefore covering a `derive_more` spelling as readily as a
  hand-written one: `Deref`, `AsRef<T>` and `Borrow<T>` with an `Ord`
  target. All three hand back a reference.
- **Inherent methods that return one**, found through their own type's
  inherent impls — a bounded query, not a search. A name pattern
  (`as_*`, `get_*`, `id`, `key`, `name`, a method named for a field)
  narrows which methods to look at, and nothing more than that: what
  admits one is
  [the cheapness test](#cheap-defined), which reads what the method
  does rather than what it is called.

**No suggested view may allocate.** The rewrite exists to delete an
allocation, so proposing `to_string`, `to_owned`, `clone` or a
`collect` to obtain a sort key gives back what it came for — and worse
than the original, since `sort_by_key` would rebuild that key once per
*comparison*. This costs nothing to honour: a comparator takes two
references and returns an `Ordering`, so an ordering never needs to
own anything. That is also the whole answer to whether `Rc::clone` and
`Arc::clone` deserve a pass for being refcount bumps rather than
allocations — they do not need one, because no comparator has to clone
at all. An `Rc<T>` element compares through `&*rc`, and if the `T`
inside is not `Ord` the clone would not have helped anyway.

The one route that does allocate is the fallback, and it is the
fallback for that reason: where the order genuinely is a rendered form
— a `Display` impl, a normalised string — `sort_by_cached_key` builds
it once per element, which is the cheapest that order can be had.
`sort_by_key` is the one to avoid there: its body is
`stable_sort(self, |a, b| f(a).lt(&f(b)))`, so the key is built twice
for every comparison, where the cached form stores one per element.

#### Cheap, defined

A return type bounds allocation at the boundary, not work: `fn
checksum(&self) -> u64` hands back a `Copy` scalar and may have hashed
a megabyte to get it, and `fn first_tag(&self) -> &str` hands back a
reference it found by scanning. A comparator runs its view O(n log n)
times, so a view that is linear in the value makes the suggestion
quadratic — a regression introduced by the rule's own advice, in a
rule whose case is partly performance.

So a view is **cheap** only when evaluating it is O(1) and
allocation-free: an expression rooted at the element, built from field
accesses, derefs and borrows, plus `match` or `if let` whose arms are
themselves that, plus calls to methods cheap by this same definition
under a depth cap. A loop, an iterator chain, an index search, a call
the rule cannot see through, or any allocation disqualifies it.

Whether that can be checked at all depends on where the method lives,
which is the same split the configuration already draws:

- **Defined in the crate being linted** — the body is there to read,
  so read it and apply the definition.
- **Defined elsewhere** — `tcx.is_mir_available` usually says no, and
  a signature is all there is. Then the naming convention carries the
  weight it was written to carry: `as_*` is documented as the cheap
  borrow, and this is a note rather than a rewrite, so trusting it
  costs a reader one glance.

Skew every uncertain call toward *expensive*. Declining a view that
was in fact cheap loses a sentence in a note, and usually degrades to
naming the fields instead, which is more explicit anyway; naming a
linear one as a sort key hands the reader a quadratic sort. The
asymmetry is not close.

What it will not do is walk free functions, trait methods at large, or
anything else that grows with the crate instead of the type: a key can
be any `fn(&T) -> K` anywhere in the program, and finding *the* one is
not a lint's job. A view that is found is named, never applied —
whether it is the order the author means stays a judgement, with
sorting versions by their rendered form as the standing
counter-example — so a wrong guess from a name pattern costs a
slightly-off sentence in a note, not a broken rewrite.

#### When no view is found

What the rule does then is `unorderable_elements`:

- `local` — the default. Report where the element is defined in the
  crate being linted, which is where deriving `Ord` or exposing a view
  is a change the reader can actually make. A report without a
  suggestion is worth its space exactly when the reader owns the type
  that withheld the order.
- `silent` — say nothing, for a project that would rather not hear
  about a round trip it has no one-line answer for.
- `all` — report foreign elements too, where the answer is an upstream
  change, a wrapper, or keeping the set. Off by default because none
  of those is a change the reader can make today.

Either reporting mode states the shape and stops: the rule has found a
round trip it cannot finish the sentence about, and says so rather
than inventing an order.

One enum rather than two booleans, deliberately. The three values are
nested scopes, not independent switches — reporting a foreign element
while staying silent about one defined next to the violation is a
combination nobody wants, and a pair of booleans would spell it.

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
   collected into a linear sequence: `Vec<U>`, `VecDeque<U>`,
   `LinkedList<U>`, or `Box<[U]>`, including the fallible forms a
   `collect` produces from an iterator of `Result` / `Option`
   (`Result<Vec<U>, E>`). A `LinkedList` landing can only ever reach
   the help-text tier, since std gives it no `sort` or `dedup` to
   suggest. `BinaryHeap` is not this shape: it is a priority
   structure whose order is its own, not a sequence whose order the
   set discarded. Nor is a map or a set:
   `keys().chain(...).collect::<BTreeSet<_>>()
   .into_iter().filter_map(...).collect::<BTreeMap<_, _>>()` ends
   somewhere a sort cannot replace.

Plus one gate: **`T: Ord`**, resolved against the element type.
Without it the suggestion does not compile, so the rule emits none —
and whether it says anything at all is `unorderable_elements`; see
[When the element is not `Ord`](#when-the-element-is-not-ord).

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
let mut unique: Vec<PackageId> =
    ids.into_iter().collect::<HashSet<_>>().into_iter().collect();

// Bound, and the adapters do not change the shape.
let sorted: BTreeSet<Platform> = targets.iter().map(Target::platform).collect();
let tier_one: Vec<Platform> = sorted.into_iter().filter(Platform::is_tier_one).collect();
```

**Prefer:**

```rust
let mut unique: Vec<PackageId> = ids.into_iter().collect();
unique.sort_unstable();
unique.dedup();

let mut platforms: Vec<Platform> = targets.iter().map(Target::platform).collect();
platforms.sort_unstable();
platforms.dedup();
let tier_one: Vec<Platform> = platforms.into_iter().filter(Platform::is_tier_one).collect();
```

Both are elements the rule rewrites unattended: a newtype over a
primitive and a derived enum each clear the applicability gate's two
clauses, and the element sweep found no size at which the round trip
beats the suggestion for either shape — 7.6× and 5.4× slower on ten
elements, still 1.35× and 1.06× at a hundred thousand.

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
directly after a sort). Everything below is quoted at
[`f607801`](https://github.com/pnpm/pnpm/commit/f60780170c962d938082562d26fdbd4689c26a85), each round
trip linked to its lines at that revision and the left-alone samples
taken from the same tree, so each stays a citation rather than a copy
that drifts.

### The round trip that sorts afterwards anyway

[`crates/cli/src/cli_args/approve_builds.rs`, L314–L324](https://github.com/pnpm/pnpm/blob/f60780170c962d938082562d26fdbd4689c26a85/crates/cli/src/cli_args/approve_builds.rs#L314-L324):

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
dropped are the ones `dedup` drops. Five lines hold the whole of it,
so nothing about the shape needs tracing across a call graph.

The vector, by contrast, is wanted, and the three callers are what
establish it. Two of them write the result to a manifest
([L129–L133](https://github.com/pnpm/pnpm/blob/f60780170c962d938082562d26fdbd4689c26a85/crates/cli/src/cli_args/approve_builds.rs#L129-L133)):

```rust
let build_packages: Vec<String> = if !packages.is_empty() {
    sort_unique(approved.clone())
} else if all {
    sort_unique(pending.to_owned())
} else {
```

and the third hands it to a prompt whose displayed order a user reads
([L247–L248](https://github.com/pnpm/pnpm/blob/f60780170c962d938082562d26fdbd4689c26a85/crates/cli/src/cli_args/approve_builds.rs#L247-L248)):

```rust
let choices = sort_unique(automatically_ignored_builds.to_vec());
match MultiSelect::new()
```

None of the three sorts, because the helper already did — which is
what makes the sort load-bearing and the set the only thing here that
is not.

Whether the work belongs in a helper at all is a separate question,
and not one the rule answers. A normalisation hidden behind a name
that does not mention it, or paid by callers that never read the
order, is worth moving to the call sites that need it — the argument
`perfectionist::needless_borrowed_parameters` makes about a signature
that borrows and then clones. It does not apply to a function called
`sort_unique` whose doc comment says it sorts and deduplicates: the
cost is in the name, every caller needs both properties, and the rule
rewrites what is inside the helper without taking a view on the helper
itself. The doc comment names the
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

[`crates/install-coordinator/src/mutation.rs`, L41–L46](https://github.com/pnpm/pnpm/blob/f60780170c962d938082562d26fdbd4689c26a85/crates/install-coordinator/src/mutation.rs#L41-L46):

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

[`crates/cli/src/cargo_deps/lockfile.rs`, L26–L35](https://github.com/pnpm/pnpm/blob/f60780170c962d938082562d26fdbd4689c26a85/crates/cli/src/cargo_deps/lockfile.rs#L26-L35):

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
single manifest declares 110.

That matters more than it looks, because the band where the round trip
wins needs *two* conditions and this consumer has only one of them.
Its element type is the one that favours the set: an npm package name
averages 20 bytes of text and a `String` compares through a pointer,
which is the whole of what the string rows measure. But the band also
needs about a thousand items, and every site the rule fires on here
handles tens. At those sizes the round trip is 1.2× to 2.2× slower, so
the advice and the faster code coincide — by size, not by type. A
crate that deduplicated the whole 1700-package lockfile would land at
roughly 0.8× instead, and would owe itself the `#[expect]`.

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

# What to do where the element has no `Ord` and no view the rule can
# find (see "When the element is not `Ord`"). `local` reports where
# the element is defined in the crate being linted; `silent` says
# nothing; `all` reports foreign elements too. Defaults to `local`:
# such a report carries no suggestion, which is worth its space where
# the reader owns the type that withheld the order and not much
# elsewhere.
unorderable_elements = "local"
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

The two autofix notes below apply to the tiers that carry a
suggestion; the third emits help text only.

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
- **Suggest `sort_unstable`, not `sort`.** This governs the sort the
  rule *introduces*; where the code already sorts, the suggestion
  keeps the call as written. Stability decides one thing: the relative order of elements that compare `Equal`. This
  rewrite has none to preserve — the value it replaces came out of a
  `HashSet`, whose order is arbitrary, or out of a `BTreeSet`, which
  had already collapsed `Equal` elements to one. So the stable sort
  guarantees something about an input order that the flagged code
  never had, and charges for it: across the tables above the stable
  sort was 1.03×–1.65× slower everywhere except at ten elements, where
  the two are a wash (0.93×–1.09×), and it allocates a scratch buffer
  where `sort_unstable` allocates nothing. It also makes
  `clippy::stable_sort_primitive` a non-issue, since that lint asks
  for exactly this. Two measured cases go the other way and neither
  changes the suggestion: an element whose comparison dwarfs its hash
  (4 KiB strings sharing a 4100-byte prefix, where the stable sort's
  lower comparison count put it at 0.98× of `sort_unstable`) belongs
  in the `#[expect]`
  case above rather than in a second suggestion, and input that is
  *already sorted* lets the stable sort's run detection pay for itself
  (`sort_unstable` at 1.01×–1.04× there) — a vector arriving sorted at
  a deduplication is not the case this rule is looking at.
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
- **Ask whether the vector earns its place.** The rewrite assumes the
  sequence at the end was wanted. That assumption is checkable against
  the landing binding's own uses — the scan the bound form already
  performs — and it decides which of two fixes is right. Where every
  use of the landing vector is a walk and nothing reads the order, the
  better fix deletes the *vector*: the set was fine and the `collect`
  after it is the waste. Where the order is read — an index, a slice
  handed to an API, a rendered list, a serialised field — the sequence
  is load-bearing and the suggestion stands as written.

  Two things keep this a note rather than a second autofix. Walking a
  `HashSet` is the nondeterminism this rule exists to remove, so
  "keep the set" is right only where nothing downstream observes the
  order, which is the judgement the `#[expect]` cases above already
  ask for. And a landing vector that escapes its body — returned,
  stored in a field — has its uses at call sites the pass cannot see,
  so the sort is the suggestion that travels.
- **Dropping a needless clone.** Where the walk is `iter().cloned()`
  or `iter().copied()`, the rewrite drops it: the set was about to be
  dropped, so the elements can be moved. Say so in the diagnostic
  rather than silently changing the chain.
- **What the diagnostic offers, and what gates it.** Naming the
  anti-pattern and writing its fix are separate questions, and the
  rule answers the second where either the measurement or the
  surrounding code settles it; prose everywhere else.

  A code suggestion, `MachineApplicable`, for the `BTreeSet` branch.
  Its output is the vector the rewrite produces, element for element
  and order for order, and it measured slower than the suggestion in every row of
  every table (1.34×–3.85× on the element-type sweep).

  A code suggestion, `MachineApplicable`, for the `HashSet` branch
  too, but only where the element makes the answer decisive: **no indirection, and a
  layout of at most 16 bytes** (`cx.layout_of`, with a reference, a
  raw pointer or a heap-owning field disqualifying). Both clauses
  carry weight, and each catches what the other misses. Size alone
  would admit `&str`, which is a 16-byte fat pointer and behaves like
  the `String` it borrows from. Indirection alone would admit
  `[u8; 32]`, which chases nothing and still came in at 0.65× on a
  hundred thousand elements, because thirty-two inline bytes are
  thirty-two bytes to compare. That is the set
  of elements the sweep found no size at which the round trip wins —
  5.4×–8.7× slower on ten elements, still 1.06×–1.79× slower on a
  hundred thousand — and it covers the primitives, the newtypes over
  them, and the derived structs and enums. A `HashSet`'s iteration
  order is unspecified, so no correct program can depend on the order
  being replaced, which is what makes the rewrite safe to apply
  unattended here.

  A code suggestion, `MachineApplicable`, for any `HashSet` element
  whatever — `String` included — **where the landing vector's next use
  is `sort()` or `sort_unstable()`**, the `sort_unique` shape. Here the
  surrounding code settles what the element could not: the set's order
  is overwritten by the next statement, so the rewrite's output matches
  the original element for element and order for order, and the set is
  left contributing a deduplication that `dedup` repeats. Keep the sort
  call the code already wrote and insert `dedup` after it rather than
  re-spelling it — this rewrite deletes a container, and swapping
  `sort` for `sort_unstable` is a separate opinion that the bullet
  below makes on its own terms.

  The follow-on sort has to order by the element's own `Ord` for this
  to hold. `sort_by`, `sort_by_key`, `sort_unstable_by`,
  `sort_unstable_by_key` and `sort_by_cached_key` order by something
  narrower than `Eq`, and `dedup` after one of them removes a strict
  subset of what the set removed: three `Item`s sharing a name, with
  versions 1, 2 and 1, come out of the set as two and out of
  `sort_by_key(|i| i.name)` + `dedup` as three, because the two equal
  elements are never adjacent. Those land in the help-text tier with
  the rest.

  Nothing about this tier is a performance claim. Where duplicates
  dominate the input the round trip plus its sort is the faster of the
  two — 0.83× at ten duplicates per entry against 1.67× at none — and
  the suggestion is offered anyway, because it preserves the meaning
  exactly while deleting a hash table, which is what
  `MachineApplicable` asks and all that it asks.

  **No code suggestion for every other `HashSet` element** — a
  `String`, a `PathBuf`, a newtype over one, a 32-byte digest.
  `sort_unstable` + `dedup` compiles there and is correct there, so
  this is not `MaybeIncorrect`; it is that the rewrite is not reliably
  an improvement, and a lint that emits one anyway is guessing with
  the reader's code. With the order unused the round trip measured
  0.68×–0.80× on every real pool, so the rewrite is a pessimisation as
  often as not, and nothing in the surrounding code says the order was
  unwanted. The 16-byte line is deliberately conservative at one edge: a
  wide struct whose comparison forwards to one small key behaves like
  that key rather than like its own size, and is held back anyway,
  because a `cmp` impl can read whatever it likes.

  What the diagnostic carries instead is the shape and the three ways
  out, for a reader who has the context the pass lacks. Keep the set,
  where nothing downstream needs a sequence — the remedy
  [When the set is the right tool](#when-the-set-is-the-right-tool)
  reaches for first. Sort, where the order is load-bearing, and put
  the sort where its cost is visible. Or `#[expect]` it, where the
  order is genuinely unused and the input is large enough for the
  round trip to pay. Any of the three is a line's work for someone who
  knows which applies, and none of them is a guess `cargo fix` could
  make.


### Why the sorted shape is not its own rule

`sort_unique` is the same anti-pattern with more evidence, not a
different one, and the three tests this repository applies to a split
all come out that way. Its trigger is a strict subset: the same build,
the same walk, the same landing, plus one more condition on what
happens next. Its configuration is the same configuration — a second
rule would read `extra_set_types` and `unorderable_elements` to decide
the identical questions. And its diagnostic is the same sentence; only
the suggestion attached to it differs.

Splitting would also cost the reader twice: two lints firing on one
expression, so a crate that wanted the shape would have to name both
in `#[expect]`, and two names for one violation, the second of which
could only be the first with a clause bolted on. What varies here is
how much the rule can prove, which is what an applicability tier is
for.

### Difficulty

**Hard, in layers that ship separately.** The trigger itself is not:
the chained form is a spine walk plus the container-type checks and a
trait-bound query, all of which a late pass has to hand, and that
layer alone is a working rule. Each layer after it is a smaller rule
of its own.

- **The bound form** adds a body-local use scan, and that scan is
  where a wrong implementation false-positives: a `contains` call, a
  borrow escaping into a closure, or a second walk each have to
  disqualify the set.
- **The applicability gate** needs `cx.layout_of` and an indirection
  walk over the element to decide which `HashSet` rewrites may be
  applied unattended.
- **The view search** is the largest: field visibility at the
  violation, a fixed set of trait queries, a name-and-return-type pass
  over inherent impls, and a body check for cheapness on local
  methods — memoised per type, and worth writing behind its own tests
  before any of it reaches a diagnostic.

A conservative first cut ships the chained form with the plain pair
and `unorderable_elements` absent, which is a rule with no search in
it at all.

## Default state

Active by default. The trigger is narrow, the `Ord` gate holds back
every suggestion the rule cannot make — reporting the shape without
one only where the reader owns the type, per `unorderable_elements` —
and the exceptions that remain are performance trade-offs a crate
states once with `#[expect]`. A crate that deduplicates large string
collections whose order nothing reads — where the measurement favours
the set — is the crate that turns the rule off in
`[perfectionist].disable`.

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
  through a `Vec` is reported by nothing: the rule that would have
  covered it was closed (see the source above). So a set round trip
  whose element fails the `Ord` gate is this rule's to mention or to
  leave, per `unorderable_elements`, rather than something it can hand
  on.
- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
