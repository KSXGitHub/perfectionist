# `splittable_adapter_closure`

**Source:** project convention, from a maintainer's review suggestion
on <https://github.com/KSXGitHub/perfectionist/pull/475>. The
workspace-manifest walk in `src/cargo_manifest.rs` was written as two
`filter_map` closures, each performing a fallible call and discarding
its error in the same step; the suggestion rewrote it so that each
adapter did one thing. Neither style guide covers this, so the wording
below is the catalogue's own rather than a quotation.

## Status

The **chain** trigger is implemented, in
[`src/rules/splittable_adapter_closure.rs`](../src/rules/splittable_adapter_closure.rs),
over `Iterator` and `DoubleEndedIterator`. What it covers:

- The adapters whose item enters by value and never comes back out,
  unary and binary, as tabled in
  [Which adapters](#which-adapters), **less** `filter_map`, `find_map`
  and `map_while`: their closure returns an `Option`, and what splits
  inside one lifts into a discipline-matched adapter rather than into a
  `map`, per [A guard and a value](#a-guard-and-a-value). Flagging them
  from here would offer the weaker split.
- The item-occurs-once rule, the two-step minimum, the position
  condition of
  [When the chain may be hoisted](#when-the-chain-may-be-hoisted), and
  the proc-macro guard.
- Liftability by a narrower test than
  [When a step can be lifted](#when-a-step-can-be-lifted) describes: a
  step lifts where its result carries no lifetime, or where the item is
  itself a reference, so a `&self` borrows the referent rather than the
  closure's local. Regions are erased in typeck results, so the
  result's lifetime cannot be matched against the item's, and this is
  the conservative pair that needs no such match.

Not implemented, and the rest of this file is their active spec:

- The **predicate** trigger of
  [Splitting a predicate](#splitting-a-predicate) and the
  **guard-and-value** trigger of
  [A guard and a value](#a-guard-and-a-value). Each is independently
  triggered, lifts into a filtering adapter chosen by discipline rather
  than into a `map`, and shares no configuration with the chain
  trigger. By the test in
  [One rule per file, one `Config` per rule](../CLAUDE.md#one-rule-per-file-one-config-per-rule)
  they are rules of their own, and whoever implements them should split
  this file rather than widen the registered lint.
- Every family beyond `Iterator`: `Option`, `Result`, `Poll`,
  `ControlFlow`, `itertools`, rayon and `pipe-trait`.
- No check that an adapter taking `&mut self` leaves its receiver in
  use. [Which adapters](#which-adapters) measures the `E0382` a split
  causes where the receiver is used after the call; the rule fires
  there all the same, so the rewrite needs the reborrow that note
  gives.
- No autofix. The diagnostic names the count and asks for the split,
  because the point-free form a split invites asks more of each step
  than the split does, so the text a rewrite would have to choose is
  not the text a reader wants.

## Statement

An adapter should do one thing. A closure doing several makes one
adapter do all of them, and the stages that would have been visible in
the pipeline are hidden inside it.

**Avoid** — a chain of steps, hidden one behind another:

```rust
let names = headers
    .map(|header| header.trim().to_ascii_lowercase())
    .collect::<Vec<_>>();
```

**Prefer** — one adapter per step, so the order the data passes through
them reads down the page:

```rust
let names = headers
    .map(str::trim)
    .map(str::to_ascii_lowercase)
    .collect::<Vec<_>>();
```

**Avoid** — a predicate built from separable tests, hidden side by
side:

```rust
let wanted = entries
    .filter(|entry| is_visible(entry) && is_recent(entry))
    .collect::<Vec<_>>();
```

**Prefer** — one adapter per test, so either can be read, moved or cut
without disturbing the other:

```rust
let wanted = entries
    .filter(is_visible)
    .filter(is_recent)
    .collect::<Vec<_>>();
```

## Why restrict this?

This is a stylistic preference, not a correctness issue. Both forms
compute the same answer, with the same laziness and the same
short-circuiting, and a closure that chains two calls is ordinary
idiomatic Rust.

The preference is that a pipeline written one stage per adapter can be
read, cut and instrumented at every stage. An `inspect` can go between
any two of them; a stage can be deleted by deleting a line; the reader
sees the same number of steps the data goes through. Folded into a
closure, those same steps are an expression to be parsed before any of
them can be found.

The split also tends to make each stage point-free, which is why the
chain example above ends at `str::trim` rather than a closure wrapping
it.
That reduction is Clippy's to enforce, not this rule's; see
[What Clippy already says](#what-clippy-already-says).

## What to lint

A closure can be doing more than one job in more than one way, and the
rule has a trigger per way. A **chain** of steps on the item splits by
lifting all but one into the item's `map`, and this section is about
that. A **predicate** built from separable tests splits into successive
filtering adapters instead, with its own trigger in
[Splitting a predicate](#splitting-a-predicate) and its own rules,
since the item there occurs once per test rather than once in all.

Flag a closure passed to one of the adapters below where the chain
rooted at its item parameter is **two or more** steps long.

The chain starts at the item parameter and runs outwards: a method
call whose receiver is the chain so far, or a call whose sole argument
is it, naming no parameter of the closure in its other arguments. It
stops at the first parent that is neither.

Where the chain may sit is constrained by the closure's shape:

- **A unary closure** — every adapter in the table below — must have
  the chain as its whole body. This is stricter than
  safety requires. It holds the scope here, and
  [Deferred: the unanchored chain](#deferred-the-unanchored-chain)
  is where it would be relaxed.
- **A binary closure** — `fold`, `try_fold`, `scan` — has the
  accumulator expression above the chain by construction, so the chain
  is a sub-expression. It may be split only where every node between
  it and the body root always evaluates its child, per
  [When the chain may be hoisted](#when-the-chain-may-be-hoisted).

Suggest one adapter per step. How many steps leave the closure differs
with its shape, because the adapter has to be left holding a closure
either way.

A **unary** closure hands its last step to the original adapter, which
is the one whose kind the pipeline depends on:

```rust
.adapter(|binding| third(second(first(binding))))
// becomes
.map(|binding| first(binding))
.map(second)
.adapter(third)
```

A **binary** closure keeps the accumulator expression, so every step
of the chain leaves and the chain's place in the body becomes the new
item parameter:

```rust
.fold(init, |acc, item| combine(acc, second(first(item))))
// becomes
.map(|item| first(item))
.map(second)
.fold(init, |acc, mapped| combine(acc, mapped))
```

A chain of only **one** step is not this rule's: there is nothing to
split, and `.map(|text| text.trim())` is already one adapter doing one
thing. Reducing it to `.map(str::trim)` is
`clippy::redundant_closure_for_method_calls`.

The item parameter must occur **exactly once**, wherever in the body
it appears. The split gives every step its own closure, so a second
occurrence is left with no binding to name:

```rust
// Not this rule's: `a` occurs twice
.map(|a| a.foo().bar().baz(a))
// The split would be
.map(|a| a.foo()).map(|x| x.bar()).map(|y| y.baz(a))
```
```
error[E0425]: cannot find value `a` in this scope
```

`.map(|x| foo(bar(baz(x)), x))` is the same shape, with the second
occurrence in an argument of the outermost call rather than of the
last one.

A binary closure's accumulator is under no such rule, because it never
leaves the closure. It may appear as often as it likes in what sits
above the chain; what it may not do is appear *in* a step, which a
leading `map` cannot reach. A step naming it is simply where the chain
stops:

```rust
// Avoid -- the chain is `trim` and `len`; `wrapping_mul` names the
// accumulator, so it is not a step, and the body keeps it
.fold(1, |acc, s| s.trim().len().wrapping_mul(acc))
// Prefer
.map(str::trim).map(str::len).fold(1, |acc, n| n.wrapping_mul(acc))
```

### Which adapters

**Adapter** here means any method that takes a closure and hands it a
value, and **item** means that value: an iterator's item, an
`Option`'s or a `Result`'s value, a `Result`'s error, a `Pipe`'s
receiver. Both words are looser than their iterator senses —
`for_each` and `fold` are consumers by that reckoning, and `pipe`
adapts nothing at all — but the shape is what this rule is about, not
the taxonomy.

A type may hand out more than one kind of item — a `Result` gives
`map` its value and `map_err` its error — and each such **channel**
has exactly one lift target, the adapter that maps that channel and
does nothing else:

| item                                            | lift target                  |
|-------------------------------------------------|------------------------------|
| an `Iterator` or `ParallelIterator` item        | `map`                        |
| the `Ok` inside a `Result` item, in `itertools` | `map_ok`                     |
| an `Option`, `Result` or `Poll` value           | `map`                        |
| a `Result` or `Poll` error                      | `map_err`                    |
| a `ControlFlow` break or continue               | `map_break` / `map_continue` |
| a `Pipe` receiver                               | `pipe`                       |

An adapter is in scope where its item **enters the closure by value
and never comes back out**. Each of these was checked by running both
forms and comparing the results:

| adapter                                      | closure                           |
|----------------------------------------------|-----------------------------------|
| `map`, `filter_map`, `flat_map`, `map_while` | `FnMut(Item) -> _`                |
| `find_map`                                   | `FnMut(Item) -> Option<B>`        |
| `for_each`, `try_for_each`                   | `FnMut(Item) -> ()` / `-> R: Try` |
| `any`, `all`, `position`, `rposition`        | `FnMut(Item) -> bool`             |

`fold`, `try_fold` and `scan` satisfy the same predicate with a
**binary** closure, where the item is the second parameter and the
first carries state across items: an accumulator taken by value for
`fold` and `try_fold`, a `&mut` state for `scan`, whose closure
returns `Option<B>` and mutates it in place. The item side splits the
same way in each, so the trigger has to find the item parameter
rather than assume the only one.

One worked example each, over the same input, with the chain `trim`
then `len` in each. Both forms of each were run and their outputs
compared:

```rust
let data = ["  a  ", " bb ", "ccc", "dddd"];   // trimmed lengths 1, 2, 3, 4
let cap = |acc: usize, n: usize| (acc + n <= 5).then_some(acc + n);
```

```rust
// fold — Avoid
let total = data.iter().fold(0, |total, s| total + s.trim().len());
// fold — Prefer
let total = data.iter().map(|s| s.trim()).map(str::len)
    .fold(0, |total, n| total + n);
// 10 either way
```

```rust
// try_fold — Avoid
let capped = data.iter().try_fold(0, |acc, s| cap(acc, s.trim().len()));
// try_fold — Prefer
let capped = data.iter().map(|s| s.trim()).map(str::len)
    .try_fold(0, |acc, n| cap(acc, n));
// `None` either way, breaking on the third item
```

```rust
// scan — Avoid
let running: Vec<_> = data.iter()
    .scan(0, |sum, s| { *sum += s.trim().len(); Some(*sum) }).collect();
// scan — Prefer
let running: Vec<_> = data.iter().map(|s| s.trim()).map(str::len)
    .scan(0, |sum, n| { *sum += n; Some(*sum) }).collect();
// [1, 3, 6, 10] either way
```

`scan` shows that the chain need not be an operand of anything: here it
sits inside a `+=` statement in a block, and what matters is only that
the block always runs it.

`DoubleEndedIterator` contributes `rfold` and `try_rfold`, the same
shape worked from the other end. They lift into `Iterator::map`, since
`Map<I, F>` is double-ended wherever `I` is: over the same input,
`rfold` gives `"ddddcccbba"` either way and `try_rfold` under the same
cap gives `None` either way. `rfind` is excluded for the reason `find`
is.

Short-circuiting does not change the count: `try_fold` breaking on the
third item ran the lifted step three times in both forms, because
`map` is lazy and one to one.

The `&mut self` adapters — `try_fold`, `try_for_each`, `any`, `all`,
`find_map`, `position`, `rposition` — leave the receiver positioned
and usable once they return. The split moves that receiver into `map`,
so code going on to use it stops compiling:

```rust
let mut it = data.iter();
let found = it.map(|s| s.trim()).map(str::len).any(|n| n > 1);
let rest: Vec<&&str> = it.collect();
```
```
error[E0382]: use of moved value: `it`
```

Suggesting `(&mut it).map(…)` reborrows instead and keeps the receiver
alive. Where the rule cannot tell, declining is the cheaper error.

### Which adapters take no leading `map`, and why

Where the item is borrowed by the closure, or handed back by the
adapter, a leading `map` does not preserve the meaning. This is not a
matter of taste: the two forms produce different answers. Over
`1..=8`, with `double` and a `keep` predicate:

| adapter      | folded               | split                    |
|--------------|----------------------|--------------------------|
| `filter`     | `[1, 2, 4, 5, 7, 8]` | `[2, 4, 8, 10, 14, 16]`  |
| `find`       | `Some(1)`            | `Some(2)`                |
| `take_while` | `[1, 2]`             | `[2, 4]`                 |
| `skip_while` | `[3, 4, 5, 6, 7, 8]` | `[6, 8, 10, 12, 14, 16]` |
| `max_by_key` | `Some(3)`            | `Some(6)`                |
| `min_by_key` | `Some(7)`            | `Some(14)`               |
| `partition`  | `[1, 2, 4, 5, 7, 8]` | `[2, 4, 8, 10, 14, 16]`  |
| `reduce`     | `Some(71)`           | `Some(72)`               |

`inspect` is excluded for the same reason without differing in its own
output: it passes the item downstream unchanged, so a leading `map`
changes what every later adapter sees.

The predicate is what the implementation should test, not the list.
The list will go stale as the standard library grows; **the item
enters by value and does not come back out** will not. `reduce` shows
why the second half is needed: its closure takes items by value, but
it returns one, so mapping first changes the result.

### Splitting a predicate

A closure can hold separable jobs without chaining anything. A
predicate that is a **conjunction** does one test and then another,
and each test can have its own adapter:

```rust
// Avoid
iter.filter(|x| foo(x) && bar(x))
// Prefer
iter.filter(foo).filter(bar)
```

This is the rule's own statement reached by a different route, and it
needs a trigger of its own, because the item occurs once **per
conjunct** — which the chain trigger forbids outright.

What it lifts into is not `map` but the adapter that filters with the
**same discipline**. These are the disciplines that turned up, and
they decide everything:

| discipline | what the adapter's answer depends on | adapters | lift target |
|---|---|---|---|
| **set** | which items satisfy the predicate, and nothing else | `filter`, `find`, `rfind`, `any`, `Option::filter`, `Option::is_some_and`, rayon `filter` / `find_first` / `any` | `filter` |
| **prefix** | the leading run that satisfies it | `take_while`, `map_while` | `take_while` |
| **neither** | the items a filter would drop, or where they sat | `all`, `Option::is_none_or`, `position`, `rposition`, `partition`, `skip_while` | none |

Measured over `1..=8`, keeping the even multiples of three:

| adapter               | folded    | split     |
|-----------------------|-----------|-----------|
| `filter`              | `[6]`     | `[6]`     |
| `find`                | `Some(6)` | `Some(6)` |
| `rfind`               | `Some(6)` | `Some(6)` |
| `any`                 | `true`    | `true`    |
| `Option::filter`      | `Some(6)` | `Some(6)` |
| `Option::is_some_and` | `true`    | `true`    |
| rayon `filter`        | `[6]`     | `[6]`     |
| rayon `find_first`    | `Some(6)` | `Some(6)` |
| rayon `any`           | `true`    | `true`    |

**Evaluation is preserved exactly**, which is not obvious and is why
this split needs no condition where
[the chain's](#when-the-chain-may-be-hoisted) does: `&&` skips
its right operand precisely where the second adapter skips the item.
The side-effect traces are identical:

```
folded  a1 a2 b2 a3 a4 b4 a5 a6 b6 a7 a8 b8
split   a1 a2 b2 a3 a4 b4 a5 a6 b6 a7 a8 b8
```

Getting the discipline wrong is not a near miss. Over `[6, 3, 12]`,
`take_while(|x| a(x) && b(x))` gives `[6]`; split into two
`take_while`s it still gives `[6]`, and split into a `filter` it gives
`[6, 12]`.

The adapters with no discipline to match were measured too:

| adapter                 | folded          | split         | why                                            |
|-------------------------|-----------------|---------------|-------------------------------------------------|
| `all`                   | `false`         | `true`        | an item failing the first conjunct turns vacuously fine |
| `Option::is_none_or`    | `false`         | `true`        | the same vacuity                                |
| `position`              | `Some(5)`       | `Some(2)`     | filtering renumbers                             |
| `partition`             | `[6]` / `[1,2]` | `[6]` / `[2]` | the right half loses what the filter dropped    |
| `skip_while`            | `[2, 3, 9]`     | `[]`          | neither target reproduces it                    |
| `\|\|` rather than `&&` | `[2,3,4,6,8]`   | `[6]`         | a disjunction is not a conjunction              |

Some of these the compiler refuses outright rather than answering
wrongly: `rposition` wants an `ExactSizeIterator` and rayon's
`position_any` an `IndexedParallelIterator`, and a `Filter` is
neither.

A family with no filtering adapter has no lift target and so no split.
`Result` is the one in scope: `r.filter(..)` is `E0599`, so
`is_ok_and` and `is_err_and` stay folded however they are written.

### A guard and a value

`filter_map` and its kin do not split on `&&`. Their closure returns
an `Option`, and what splits is the chain of `Option` combinators
inside it: each has an iterator adapter that does the same thing, and
the split hands the work over.

| inside the closure         | becomes      |
|----------------------------|--------------|
| `Option::and_then`         | `filter_map` |
| `Option::map`              | `map`        |
| `Option::filter`           | `filter`     |
| `bool::then` / `then_some` | `filter`, with the value going on to `map` |

```rust
// Avoid
iter.filter_map(|x| parse(x).and_then(validate))
// Prefer
iter.filter_map(parse).filter_map(validate)

// Avoid
iter.filter_map(|x| is_wanted(x).then(|| render(x)))
// Prefer
iter.filter(is_wanted).map(render)
```

Measured over `1..=8`: `and_then` gives `[61]` either way, `map`
`[120, 140, 160, 180]`, `filter` `[60, 80]`, and `then_some`
`[20, 40, 60, 80]`. `find_map` carries the same correspondence,
`Some(61)` either way.

**The lifted adapter has to share the discipline of the one it goes in
front of.** `filter_map` is set-shaped, so it may precede another
set-shaped adapter and not a prefix-shaped one. Over `[6, 3, 12]`, a
`map_while` whose closure ends in `and_then` gives `[61]` folded and
`[61, 121]` split, because the lifted `filter_map` drops the very item
that would have stopped it. `map_while`'s own guard splits into a
`take_while` instead:

```rust
// Avoid
iter.map_while(|x| is_wanted(x).then(|| render(x)))
// Prefer
iter.take_while(is_wanted).map(render)
```

Over `[6, 3, 12]` that gives `[60]` folded, `[60]` through
`take_while`, and `[60, 120]` through `filter`.

### `Option` and `Result`

Both hand the closure a value that does not come back out, so both
split — with `map` as the lift target on the value channel and
`map_err` on the error channel. Measured folded against split:

| adapter                  | folded       | split        |
|--------------------------|--------------|--------------|
| `Option::map`            | `Some("42")` | `Some("42")` |
| `Option::map` on `None`  | `None`       | `None`       |
| `Option::and_then`       | `Some(42)`   | `Some(42)`   |
| `Option::map_or`         | `43`         | `43`         |
| `Option::map_or_else`    | `43`         | `43`         |
| `Option::is_some_and`    | `true`       | `true`       |
| `Result::map`            | `Ok("42")`   | `Ok("42")`   |
| `Result::and_then`       | `Ok(43)`     | `Ok(43)`     |
| `Result::is_ok_and`      | `true`       | `true`       |
| `Result::map_err`        | `Err("14")`  | `Err("14")`  |
| `Result::or_else`        | `Ok(15)`     | `Ok(15)`     |
| `Result::unwrap_or_else` | `15`         | `15`         |
| `Result::is_err_and`     | `true`       | `true`       |

`Option::filter` and `Option::inspect` are excluded for the reason
`Iterator::filter` and `Iterator::inspect` are: the value comes back
out, so a leading `map` changes what does. Over `Some(2)` and a
doubling step, both give `Some(2)` folded and `Some(4)` split.
`Result::inspect` and `Result::inspect_err` go the same way.

`Result::map_or_else` takes **one closure per channel** — the first on
the error, the second on the value — so the adapter table is keyed by
argument position rather than by method name alone. Lifting the error
one into `map_err` and leaving the other gave `"14"` either way.

A closure handed nothing — `Option::unwrap_or_else`,
`Option::or_else`, `ok_or_else` — has no item to root a chain at, so
this rule does not reach it.

The value channel, with `map` as the lift target:

```rust
// Avoid
maybe_id.map(|id| render(double(id)))
// Prefer
maybe_id.map(double).map(render)
```

The error channel, with `map_err`:

```rust
// Avoid
outcome.map_err(|code| render(double(code)))
// Prefer
outcome.map_err(double).map_err(render)
```

### `itertools`

`Itertools` is a blanket extension of `Iterator`, so most of what it
adds lifts into `Iterator::map` like the rest. Measured folded against
split:

| adapter         | folded                          | split                           |
|-----------------|---------------------------------|---------------------------------|
| `map_ok`        | `[Ok(1), Ok(2), Err(7), Ok(4)]` | `[Ok(1), Ok(2), Err(7), Ok(4)]` |
| `filter_map_ok` | `[Ok(0), Ok(1), Err(7), Ok(3)]` | `[Ok(0), Ok(1), Err(7), Ok(3)]` |
| `fold_ok`       | `Err(7)`                        | `Err(7)`                        |
| `fold_while`    | `6`                             | `6`                             |
| `counts_by`     | `[(1, 1), (2, 1), (3, 1), (4, 1)]` | `[(1, 1), (2, 1), (3, 1), (4, 1)]` |
| `partition_map` | `[2, 4]` / `[1, 3]`             | `[2, 4]` / `[1, 3]`             |

The `*_ok` family is why a lift target is worth naming per channel
rather than per type. Their closure is handed the `Ok` *inside* a
`Result` item, so it lifts into `map_ok` rather than `map` — a channel
nested one level inside the iterator's own. `fold_ok` is binary over
that same nested item; `fold_while` is binary over the iterator's.

Excluded, measured the same way: `unique_by` gives `[2, 3]` folded
against `[4]` split, `tree_reduce` `Some(16)` against `Some(32)`, and
`sorted_by_key` `[4, 3, 2]` against `[8, 6, 4]`. They join `filter_ok`,
`update`, `find_position`, `into_group_map_by`, `position_max_by_key`
and `position_min_by_key`, each of which takes its item by reference
and so fails the predicate's first half before the second is reached.

The nested channel is the one worth seeing, since `map_ok`'s item sits
inside the iterator's own:

```rust
// Avoid
lines.map_ok(|line| line.trim().len())
// Prefer
lines.map_ok(str::trim).map_ok(str::len)
```

### `Poll`, `ControlFlow` and `[T; N]`

Small functors, each splitting on every channel it has. Measured
folded against split:

| adapter                     | folded             | split              |
|-----------------------------|--------------------|--------------------|
| `Poll::map`                 | `Ready("42")`      | `Ready("42")`      |
| `Poll::map` on `Pending`    | `Pending`          | `Pending`          |
| `Poll::map_ok`              | `Ready(Ok("42"))`  | `Ready(Ok("42"))`  |
| `Poll::map_err`             | `Ready(Err("14"))` | `Ready(Err("14"))` |
| `ControlFlow::map_break`    | `Break("42")`      | `Break("42")`      |
| `ControlFlow::map_continue` | `Continue("14")`   | `Continue("14")`   |
| `<[T; N]>::map`             | `["2", "4", "6"]`  | `["2", "4", "6"]`  |

`ControlFlow` is `Result`'s shape without the error convention: two
channels, each its own lift target, each leaving the other alone —
`map_break` over a `Continue(7)` gives `Continue(7)` folded and split
alike. `Poll` carries the value channel on `Poll<T>`, and both
channels on `Poll<Result<T, E>>` and `Poll<Option<Result<T, E>>>`.

`[T; N]::map` is **excluded**, and its measured row above is why the
exclusion is needed rather than why it would be in scope. It is
**eager**, so the split does not merely materialise an intermediate
`[U; N]` — it runs every `f` before any `g`, where the folded form
alternated them:

```
array folded  f1 g2 f2 g4 f3 g6
array split   f1 f2 f3 g2 g4 g6
iter  folded  f1 g2 f2 g4 f3 g6
iter  split   f1 g2 f2 g4 f3 g6
```

Pure steps give the same answer either way, which is what the table's
row shows. Impure ones do not, and the reordering compiles, which is
the quiet kind of divergence that
[When the chain may be hoisted](#when-the-chain-may-be-hoisted)
describes. It has the same shape: the split is safe exactly where the
steps are pure, and purity is what no lint can ask for. Where every
other family lifts into a lazy `map` that keeps `f` and `g` interleaved
per item, an eager one cannot.

So the condition is that **the lift target be lazy**, which excludes
`[T; N]::map` and nothing else in scope. Admitting it needs a purity
test instead;
[Deferred: arrays behind a purity test](#deferred-arrays-behind-a-purity-test)
is where that sits. `try_map` is `#[unstable]` and out of reach either
way.

`Poll`, on its value channel:

```rust
// Avoid
poll.map(|id| render(double(id)))
// Prefer
poll.map(double).map(render)
```

`ControlFlow`, on whichever channel the closure was handed, the other
left alone:

```rust
// Avoid
flow.map_break(|code| render(double(code)))
// Prefer
flow.map_break(double).map_break(render)
```

### Rayon's parallel adapters

`rayon::iter::ParallelIterator` has the same shape and takes the same
split, so this rule reaches it too:

```rust
// Avoid
lines.par_iter().map(|line| line.trim().len())
// Prefer
lines.par_iter().map(|line| line.trim()).map(str::len)
```

Measured over the same input, folded against split:

| adapter          | folded         | split          |
|------------------|----------------|----------------|
| `map`            | `[1, 2, 3, 4]` | `[1, 2, 3, 4]` |
| `filter_map`     | `[0, 1, 2, 3]` | `[0, 1, 2, 3]` |
| `any` / `all`    | `true`         | `true`         |
| `find_map_first` | `Some(0)`      | `Some(0)`      |
| `position_any`   | `Some(2)`      | `Some(2)`      |
| `fold`           | `10`           | `10`           |
| `try_fold`       | `Some(10)`     | `Some(10)`     |

Rayon performs the lift itself: its default `any` is
`self.map(predicate).find_any(bool::clone).is_some()`.

Its exclusions mirror the sequential ones, measured the same way:
`filter` gives `[1, 2, 4, 5, 7, 8]` against `[2, 4, 8, 10, 14, 16]`,
`max_by_key` `Some(4)` against `Some(14)`, and `reduce` `306` against
`612`.

The differences from the sequential set:

- **The names.** There is no `scan`, no `map_while` and no
  `rposition`; there is `position_any`, and `find_map_any` /
  `find_map_first` / `find_map_last` in place of `find_map`. `fold`
  yields per-chunk accumulators rather than one value, and its item
  side splits all the same.
- **A lifted step's result must be `Send`.** `ParallelIterator::map`
  requires it of the item it produces; the folded form does not,
  because the value never leaves the closure:

  ```rust
  .map(|s| Rc::new(*s).len())               // compiles
  .map(|s| Rc::new(*s)).map(|r| r.len())    // does not
  ```
  ```
  error[E0277]: `Rc<&str>` cannot be sent between threads safely
  ```

- **The receiver is never lost.** Rayon's consumers take `self`, so
  none of them carry the `&mut self` hazard in
  [Which adapters](#which-adapters).

### `pipe-trait`'s piping methods

`pipe_trait::Pipe` is `impl<X> Pipe for X {}`, and `pipe` is defined
as `f(self)`, so the split here is an identity rather than a
measurement: `x.pipe(|v| g(f(v)))` and `x.pipe(f).pipe(g)` are both
`g(f(x))`. Measured anyway, over `21` with a doubling and a
formatting step: `"42"` either way.

The rewrite differs from the iterator family in which end keeps the
original method. There the leading steps become `map` and the adapter
stays last, because its kind is what the pipeline depends on. Here
there is no last step to keep: each one becomes a piping method, and
**which method is decided by how the folded body reached its
argument**.

| the step's parameter                         | methods that carry it                |
|----------------------------------------------|--------------------------------------|
| `Self`, by value                             | `pipe`                               |
| `&Self`                                      | `pipe_ref`, `pipe_borrow`            |
| `&mut Self`                                  | `pipe_mut`, `pipe_borrow_mut`        |
| `&Param`, for a `Param` the receiver converts to | `pipe_as_ref`, `pipe_deref`, `pipe_borrow` |
| `&mut Param`                                 | `pipe_as_mut`, `pipe_deref_mut`, `pipe_borrow_mut` |

The mapping is not one to one, and the rule has to choose. Every
method was tried against a by-value, a by-reference and a
by-`&str` step, on a receiver implementing `Deref<Target = str>`,
`AsRef<str>` and `Borrow<str>` alike: each cell either produced the
same output as every other or failed with `E0631`, never a different
answer. Where more than one method fits, they agree, and which to
suggest is a style question rather than a correctness one.

`pipe_borrow` spans more than one row because `Borrow` is reflexive —
`impl<T> Borrow<T> for T` — so it can hand over `&Self` as readily as
a `&Param`, and it was the only method to accept more than one of
the steps tried.

So a body mixing them splits into a chain that mixes them, each step
taking its argument the way the body did:

```rust
// Avoid
value.pipe(|x| foo(&bar(baz(&x))))
// Prefer
value.pipe(|x| baz(&x)).pipe(bar).pipe_ref(foo)
```

Measured `6` either way, as is the `Deref` form of the same shape,
`value.pipe_deref(baz_str).pipe(bar).pipe_ref(foo)`.

**The head is the exception, and a destructor is why.** Writing that
first step as `value.pipe_ref(baz)` also typechecks and also gives
`6`, but `pipe` moves the receiver into the closure while `pipe_ref`
only borrows it, so the receiver stops being dropped at the end of the
step and is dropped at the end of its scope instead:

```
folded                        drop a / after
split, head kept as `pipe`    drop b / after
split, head as `pipe_ref`     after / drop c
```

It compiles, and nothing in the result changes, which makes it
another divergence of the quiet kind
[When the chain may be hoisted](#when-the-chain-may-be-hoisted)
describes. Keeping the head a
`pipe` whose closure takes the reference inside costs one closure and
preserves the drop point; every step after the head is past the
receiver and free to name whichever method its argument wants.

Every method on the trait was run in both forms, the by-value one over
a doubling and a formatting step, the rest over a trimming or sorting
step and a length:

| method            | receiver       | folded | split  |
|-------------------|----------------|--------|--------|
| `pipe`            | `Self`         | `"42"` | `"42"` |
| `pipe_ref`        | `&Self`        | `2`    | `2`    |
| `pipe_mut`        | `&mut Self`    | `3`    | `3`    |
| `pipe_as_ref`     | `&Param`       | `2`    | `2`    |
| `pipe_as_mut`     | `&mut Param`   | `3`    | `3`    |
| `pipe_deref`      | `&Param`       | `2`    | `2`    |
| `pipe_deref_mut`  | `&mut Param`   | `3`    | `3`    |
| `pipe_borrow`     | `&Param`       | `2`    | `2`    |
| `pipe_borrow_mut` | `&mut Param`   | `3`    | `3`    |

The `_as_`, `_deref` and `_borrow` families differ from `pipe_ref` and
`pipe_mut` only in which conversion reaches the closure — `AsRef`,
`Deref`, `Borrow` and their mutable counterparts. That conversion
happens once, at the head, and the steps above it neither see it nor
care, so the split is the same in all of them.

**Which method heads the chain decides whether a lifted step may
borrow.** `pipe` consumes the receiver, so a step returning a borrow
of it is the `E0515` of
[When a step can be lifted](#when-a-step-can-be-lifted):

```rust
String::from("  x  ").pipe(|v| v.trim()).pipe(str::len)
```
```
error[E0515]: cannot return value referencing function parameter `v`
```

Every other method hands the closure a borrow carrying the
receiver's own lifetime, so the same lift compiles: `pipe_ref`,
`pipe_deref` and `pipe_borrow` each gave `1` where `pipe` gave
`E0515`. So the borrow test this rule applies everywhere is answered
here by the head of the chain rather than by the step.

Nothing in `Pipe` is excluded. It has one shape, and no sibling that
hands the value back.

`pipe` shows the hoisting condition most starkly, since a `pipe` runs
once rather than once per item. Folded, the parse below never runs;
split, it panics:

```rust
"zz".pipe(|t| if flag { t.parse::<i32>().unwrap() } else { 0 })
```
```
folded=0
thread 'main' panicked: called `Result::unwrap()` on an `Err` value: ParseIntError
```

### When a step can be lifted

Splitting is sound only where the lifted step does not hand back a
borrow of the binding. `filter_map` gives the closure an owned item
that dies at the end of the step, so a lifted call returning a
reference into it is `E0515`:

```rust
.filter_map(|manifest| manifest.get("workspace"))
```
```
error[E0515]: cannot return value referencing function parameter `manifest`
```

Taking the receiver by value rules this out, since a consumed value
cannot be borrowed from. Taking it by reference does not always cause
it — `Path::join` takes `&self` and returns an owned `PathBuf`, and
lifts fine. A conservative implementation lifts where the step's
result type contains no lifetime derived from the binding, and
declines otherwise.

The *point-free* form asks for more than the split does. `.map(T::m)`
needs `m` to take `self` by value and take no arguments besides the
receiver, or the path is not a function of one argument and the item
does not fit it:

```rust
.map(Path::components)   // error[E0631]: expected function signature `fn(PathBuf) -> _`
```

So the rule suggests a split, in a closure where a path will not do,
and leaves the reduction to Clippy.

### When the chain may be hoisted

A lifted step runs once per item the adapter pulls. A step left in the
closure runs once per item only where the closure always evaluates it,
so a chain sitting in a conditionally-evaluated position changes
behaviour when hoisted — and the change compiles:

```rust
// `flag` is false, and the strings do not parse
.map(|s| if flag { s.trim().parse::<usize>().unwrap() } else { 0 })
// hoisted
.map(|s| s.trim()).map(|s| s.parse::<usize>().unwrap()).map(|n| if flag { n } else { 0 })
```
```
folded=[0, 0]
thread 'main' panicked: called `Result::unwrap()` on an `Err` value: ParseIntError
```

A wrong suggestion from this rule is usually a compile error. This one
is not: it compiles, and it behaves differently. So a chain is split
only where every node between it and the body root always evaluates
the child the chain came from:

| the chain's parent                                                      | always evaluates it |
|-------------------------------------------------------------------------|---------------------|
| a binary operator other than `&&` / `\|\|`, or a unary one                | yes                 |
| the right-hand side of an assignment, compound (`+=`) or plain           | yes                 |
| a cast, a field, an index, a reference                                   | yes                 |
| a tuple, array, struct or repeat expression                              | yes                 |
| an argument or the callee of a call                                      | yes                 |
| the scrutinee of a `match`, the condition of an `if`                     | yes                 |
| a block's tail, or the initialiser of a `let` in it                      | yes                 |
| the operand of a `break` or `return`                                     | yes                 |
| an arm of a `match` or `if`, `?` among them, since it desugars to a `match` | no               |
| the right operand of `&&` or `\|\|`                                       | no                  |
| the body of a nested closure                                             | no                  |
| the body of a loop                                                       | no                  |
| anything else                                                            | treat as no         |

The condition is sufficient rather than necessary: a pure step in a
conditional position would hoist safely, and this declines it. Rust
exposes no purity or no-panic test a lint could ask instead.

None of this depends on which adapter the closure was passed to, so
the condition reads the same for every family in scope.

### What Clippy already says

Measured on Clippy 1.94, at default levels Clippy says nothing about
any of this. Its lints meet this rule at the edges:

- `clippy::redundant_closure` (`style`, warn-by-default) reduces
  `|text| trim(text)` to `trim`, completing a split this rule made.
- `clippy::redundant_closure_for_method_calls` (`pedantic`,
  allow-by-default) does the same for a method call, suggesting
  `T0::bar` for `|value| value.bar()`.

Nothing in Clippy splits a chained closure, which is the part this
rule is for.

### Exemptions

Do *not* flag:

- A **unary** closure whose body is not the chain — a block with
  statements, a `match`, a `?`, or an operator expression — per the
  anchor in [What to lint](#what-to-lint).
- A chain whose position is not always evaluated, per
  [When the chain may be hoisted](#when-the-chain-may-be-hoisted).
- A body where the item parameter occurs more than once, per
  [What to lint](#what-to-lint).
- A step whose result borrows from the binding, per
  [When a step can be lifted](#when-a-step-can-be-lifted).
- A receiver used again after the adapter returns, per
  [Which adapters](#which-adapters).
- A closure produced by a macro expansion, where the suggestion would
  be written into the macro body.
- A step whose result is not `Send`, under a rayon adapter, per
  [Rayon's parallel adapters](#rayons-parallel-adapters).
- An adapter whose lift target is eager, which reorders the steps it
  separates — `[T; N]::map`, per
  [`Poll`, `ControlFlow` and `[T; N]`](#poll-controlflow-and-t-n).
- A predicate whose adapter matches no filtering discipline, or whose
  family has no filtering adapter at all, per
  [Splitting a predicate](#splitting-a-predicate). A `||` is not a
  conjunction, and does not split either.
- An adapter whose item this rule has no lift target for. The families
  it does reach are the ones tabled in
  [Which adapters](#which-adapters); `Ref`, `RefMut` and `Pin` have a
  `map` the same reasoning would carry to, and this rule does not
  reach them.

These shapes are declined although they do split, because the rewrite
they need is not the one this rule makes. Both forms of each were run
and their outputs compared:

- **A destructured item parameter.** `fold(0, |acc, (_k, v)| acc +
  v.trim().len())` splits to `.map(|(_k, v)| v.trim()).map(str::len)`,
  reproducing the pattern in the lifted `map`. The chain bottoms out
  at a binding the pattern introduced rather than at the parameter, so
  the walk stops before it starts.
- **A `?` in the body.** `try_fold(0, |acc, s| Ok(acc +
  s.trim().parse::<i32>()?))` splits to
  `.map(str::trim).map(|s| s.parse::<i32>()).try_fold(0, |acc, r| Ok(acc + r?))`,
  which changes the item type to `Result` and leaves the `?` behind.
  That is a different rewrite from lifting a step.

## Interaction with sibling rules

`perfectionist::overly_long_method_chain`
([`src/rules/overly_long_method_chain.rs`](../src/rules/overly_long_method_chain.rs))
counts a chain's distinct calls against a limit. The two rules narrow
each other rather than contradict: code satisfying both is a smaller
set than code satisfying either, and not an empty one.

The cost falls on alternation rather than on length, because a run of
the same method counts once. Measured against the default `max_calls`
of 5, with five stages either way:

- five consecutive `map`s: silent, counting three distinct calls.
- `map` / `filter_map` / `map` / `filter_map` / `map`: **seven**
  distinct calls, flagged.

So this rule is free where it produces a run and expensive where it
interleaves adapter kinds. Where it does cost, the chain rule's
`NAMING_HELP` offers remedies, and each leaves this rule with nothing
to fire on:

- **Bind a stage to a `let`.** Two shorter chains, each still one step
  per adapter. `CUT_HELP` declines a cut whose only available name
  would describe the steps rather than the value, and the names it
  gives as examples are the ones taken from the adapter kind rather
  than from what the stage produces. A cut after `.map(str::trim)` is
  called `trimmed`, which is the value.
- **Move a run of stages into a function.** This rule fires on a
  closure passed to an adapter, so `items.map(normalise)` is a path
  and no trigger at all.

Both look like this, on the chain the Statement opens with:

```rust
// Naming a stage
let trimmed = headers.map(str::trim);
let names = trimmed.map(str::to_ascii_lowercase).collect::<Vec<_>>();

// Or moving the run into a function, which this rule cannot see at all
fn normalise(header: &str) -> String { header.trim().to_ascii_lowercase() }
let names = headers.map(normalise).collect::<Vec<_>>();
```

Extraction is the one to take knowingly. It satisfies both triggers
while putting the steps back inside a body, which is the arrangement
this rule exists to open up: the pipeline gets shorter and its stages
go somewhere else. Extraction earns its place where the extracted run
has a name of its own, and costs something where it is reached for
only to get under a limit.

[`pipe_style`](./pipe-style.md) governs where a pipe may sit in a
chain rather than what its closure holds, so their triggers are
disjoint. Its `pipe_at_chain_boundary` sub-check forbids a
`value.pipe(f)` that neither continues a method chain nor is continued
by one — which the split satisfies by construction, since splitting
leaves a `.pipe(…)` followed by another.

## Configuration

None. There is one direction, and the triggers agree on it.
Whether a project wants the rule at all is the `[perfectionist]`
`disable` decision, not a knob.

## Implementation notes

A `LateLintPass`, because the trigger needs types: the receiver has to
be an `Iterator`, and the liftability of each step depends on what its
result borrows from.

- `check_expr` on `ExprKind::MethodCall` whose segment names one of
  the adapters, gated by the trait it resolves to:
  `clippy_utils::is_trait_method(cx, expr, sym::Iterator)` for the
  sequential set, and a `DefPath` match for
  `itertools::Itertools`, `rayon::iter::ParallelIterator` and
  `pipe_trait::Pipe`, none of which carries a diagnostic item to ask
  for instead. `Option`, `Result`, `Poll` and `ControlFlow` carry
  inherent methods rather than trait ones, so those are the receiver's
  own diagnostic item and the method name.
- Key the adapter table by method **and argument position**: the
  position says which argument holds a closure, which channel its
  parameter comes from and so which adapter the lift targets, and
  whether the item is the closure's only parameter.
  `Result::map_or_else` needs all of that, carrying one closure per
  channel.
- Under a rayon adapter, require each lifted step's result to be
  `Send` as well, per
  [Rayon's parallel adapters](#rayons-parallel-adapters).
- Find the item parameter's occurrence in the body, then walk *up*
  through `parent_hir_node`: a `MethodCall` whose receiver is the
  chain so far and whose arguments name no parameter of the closure,
  or a `Call` whose sole argument is it. Stop at the first parent that
  is neither.
- Count the item parameter's occurrences directly rather than leaving
  it to the walk: the clauses above do not constrain everything a body
  can hold — a `Call`'s callee expression, for one. The count has to
  see inside nested closures, where a step's closure argument can name
  the item or the accumulator.
- "Sole argument" is deliberately conservative. `foo(baz(x), 1)` does
  split, as `.map(baz).map(|v| foo(v, 1))`, but recognising it means
  picking which argument is the chain, and picking wrong suggests code
  that does not compile. A missed finding is the cheaper error.
- Two or more steps is the trigger; one step is not.
- Decide liftability from each step's result type: decline where it
  carries a lifetime derived from the receiver. Taking the receiver by
  value is the easy sufficient condition. A unary closure's last step
  is exempt, since it stays with the adapter rather than moving into a
  `map`.
- Then check where the chain sits, per
  [When the chain may be hoisted](#when-the-chain-may-be-hoisted). For
  a binary closure that means walking from the chain's top to the body
  root. For a unary closure, require the chain's top to *be* the body
  root, which satisfies the condition vacuously.
- Suppress proc-macro-synthesised nodes per
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).
  The diagnostic span is the adapter's method segment, which a derive
  can give a user-source span, so the guard is needed and a
  `ui/<rule>_proc_macro.rs` fixture should prove it.
- Every Avoid example in this file becomes a fixture the rule fires on,
  and every Prefer example one it stays silent on. The Prefer half is
  the one to insist on: a form this rule asks for and then flags again
  is a false positive, and no amount of reading the Avoid cases finds
  it. The families reached through a dependency — rayon, itertools,
  `pipe-trait` — take a stub under `ui/auxiliary/` rather than the
  crate, the way
  [`ui/auxiliary/command_extra.rs`](../ui/auxiliary/command_extra.rs)
  and [`ui/auxiliary/clap.rs`](../ui/auxiliary/clap.rs) already do.

### Difficulty

**Medium.** The walk and the adapter table are easy. Liftability and
position are what raise it. Liftability decides whether a suggestion
*compiles*: a borrow of the binding is what separates one that does
from one that does not, and the conservative answer has to be the one
that declines. Position decides whether a suggestion is *correct*, and
is the only place in this rule where being wrong produces a rewrite
that compiles and behaves differently — which is why it is
exercised on the binary adapters alone here. Rayon costs a little more
again: its trait has no diagnostic item, so it is matched by path, and
a fixture for it needs a stub of the trait rather than the crate. A
first implementation may restrict itself to `Iterator`, and to steps
taking the receiver by value, and leave the rest unflagged — a real
subset rather than a token one.

## Default state

Active by default.

None of the grounds
[the activation model](./IMPLEMENTATION_CONVENTIONS.md#rule-activation-model)
reserves `Inactive` for holds here. The shape this rule flags is
ordinary idiomatic Rust, which is a reason to expect plenty of
diagnostics rather than a reason to ship the rule off: firing on what
the rule names is not a false positive.

## Deferred: the unanchored chain

The anchor on unary closures is scope containment, not soundness: a
chain that satisfies
[When the chain may be hoisted](#when-the-chain-may-be-hoisted) is
safe to split wherever it sits, and the binary adapters already rely
on that. Dropping the anchor is one condition fewer in the trigger,
and it would newly flag a unary closure with something above its
chain:

| above the chain      | example                                                     |
|----------------------|-------------------------------------------------------------|
| an operator          | `.map(\|x\| x.trim().len() + 1)`                              |
| an argument          | `.map(\|x\| format!("{}", x.trim().to_uppercase()))`          |
| a block's statements | `.map(\|x\| { let n = x.trim().len(); n * 2 })`               |
| a `match` scrutinee  | `.map(\|x\| match x.trim().len() { 0 => None, n => Some(n) })` |

A conditional position is not on that list. The hoisting condition
excludes it for a unary closure exactly as it does for a binary one,
so the widening adds findings without adding the one hazard a
compiler does not catch.

Before it is taken:

- **Measure how often those shapes occur.** They are not measured
  here, and this rule's own rationale is that the shape it flags is
  already idiomatic, so a widening has to be worth arguing for.
- **Decide what an upgrade does.** A project already running the rule
  would get more findings from the same configuration, and since it
  ships on, that is every consumer. Either that is
  accepted and said out loud, or the wider trigger arrives as a
  configuration value — which would be this rule's first, against a
  catalogue that prefers a rule with one direction over a knob.

The destructured item parameter that
[Exemptions](#exemptions) declines is *not* part of this: that one is
about where the chain bottoms out, and the anchor is about where it
stops at the top.

## Deferred: arrays behind a purity test

`[T; N]::map` is excluded for reordering the steps it separates, and
reordering only matters where a step has an effect to reorder. A rule
that could ask "is this step pure?" would admit arrays, and would also
loosen
[When the chain may be hoisted](#when-the-chain-may-be-hoisted), which
declines a conditional position for the same unaskable reason.

What the repository has is not that test. `impure_macro_arguments`
carries a purity walker, but it reads `TokenTree`s in a pre-expansion
pass and answers from a configured list of pure-getter names, where
this rule is a late pass holding HIR and types. `clippy_utils` offers
`eager_or_lazy::switch_to_eager_eval`, which is HIR-level but answers
a different question — whether an expression is cheap *and* effect-free
enough to move, which is a heuristic tuned for
`unnecessary_lazy_evaluations` rather than a purity oracle. Borrowing
either for this would be claiming an answer neither gives.

So the work is: build an HIR purity predicate, decide where it lives
per
[the crate-internal helper conventions](../CLAUDE.md#one-rule-per-file-one-config-per-rule),
and settle what it may assume — whether a call to a `const fn` counts,
whether a trait method may be judged from its signature, and what it
does with a closure it cannot see through. Until then the conservative
answer stands: the lift target must be lazy, and a conditional
position is declined.
