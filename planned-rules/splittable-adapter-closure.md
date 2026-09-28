# `splittable_adapter_closure`

**Source:** project convention, from a maintainer's review suggestion
on <https://github.com/KSXGitHub/perfectionist/pull/475>. The
workspace-manifest walk in `src/cargo_manifest.rs` was written as two
`filter_map` closures, each performing a fallible call and discarding
its error in the same step; the suggestion rewrote it so that each
adapter did one thing. Neither style guide covers this, so the wording
below is the catalogue's own rather than a quotation.

## Statement

An iterator adapter should do one thing. A closure whose body chains
several steps makes one adapter do all of them, and the stages that
would have been visible in the pipeline are hidden inside it.

**Avoid:**

```rust
let names = headers
    .map(|header| header.trim().to_ascii_lowercase())
    .collect::<Vec<_>>();
```

**Prefer:**

```rust
let names = headers
    .map(str::trim)
    .map(str::to_ascii_lowercase)
    .collect::<Vec<_>>();
```

## Why restrict this?

This is a stylistic preference, not a correctness issue. Both forms
build the same iterator, with the same laziness and the same
short-circuiting, and a closure that chains two calls is ordinary
idiomatic Rust.

The preference is that a pipeline written one stage per adapter can be
read, cut and instrumented at every stage. An `inspect` can go between
any two of them; a stage can be deleted by deleting a line; the reader
sees the same number of steps the data goes through. Folded into a
closure, those same steps are an expression to be parsed before any of
them can be found.

The split also tends to make each stage point-free, which is why the
example above ends at `str::trim` rather than a closure wrapping it.
That reduction is Clippy's to enforce, not this rule's; see
[What Clippy already says](#what-clippy-already-says).

## What to lint

Flag a closure passed to one of the adapters below where the chain
rooted at its item parameter is **two or more** steps long.

The chain starts at the item parameter and runs outwards: a method
call whose receiver is the chain so far, or a call whose sole argument
is it, naming no parameter of the closure in its other arguments. It
stops at the first parent that is neither.

Where the chain may sit is constrained by the closure's shape:

- **A unary closure** — every adapter in the table below — must have
  the chain as its whole body. This is stricter than
  safety requires. It holds this stage's scope, and
  [Later stage: the unanchored chain](#later-stage-the-unanchored-chain)
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
// The chain is `trim` and `len`. `wrapping_mul` names the
// accumulator, so it is not a step, and the body keeps it.
.fold(1, |acc, s| s.trim().len().wrapping_mul(acc))
// becomes
.map(str::trim).map(str::len).fold(1, |acc, n| n.wrapping_mul(acc))
```

### Which adapters

An adapter can take a leading `map` only where the item **enters the
closure by value and never comes back out**. Each of these was checked
by running both forms and comparing the results:

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

`DoubleEndedIterator` contributes `rfold` and `try_rfold`, the same
shape worked from the other end. They lift into `Iterator::map`, since
`Map<I, F>` is double-ended wherever `I` is: `rfold` over the input
below gives `"ddddcccbba"` either way, and `try_rfold` under the same
cap gives `None` either way. `rfind` is excluded for the reason `find`
is.

One worked example each, over the same input, with the chain `trim`
then `len` in all three. Both forms of each were run and their outputs
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

### Which adapters are excluded, and why

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

### Rayon's parallel adapters

`rayon::iter::ParallelIterator` has the same shape and takes the same
split, so this rule reaches it too. Measured over the same input,
folded against split:

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

Its exclusions mirror the sequential ones and were measured the same
way — `filter` gives `[1, 2, 4, 5, 7, 8]` against
`[2, 4, 8, 10, 14, 16]`, `max_by_key` `Some(4)` against `Some(14)`,
and `reduce` `306` against `612`.

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
  none of them carry the `&mut self` hazard above.

The hoisting condition carries over as it stands: a `map` runs per
item where a branch does not, and that reasoning does not depend on
which trait the adapter belongs to.

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

This is the only divergence in this rule that a compiler does not
catch, so a chain is split only where every node between it and the
body root always evaluates the child the chain came from:

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
- An adapter belonging to neither `Iterator` nor `ParallelIterator`.
  `Option`, `Result`, `[T; N]`, `Poll`, `ControlFlow` and `Ref` each
  have a `map` of their own, and the same reasoning would carry to
  them; this rule does not reach them.

These shapes are declined although they do split, because the rewrite
they need is not the one this rule makes. Each was run in both forms
and agreed:

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
counts a chain's distinct calls against a limit. The two rules can
both be satisfied, but the code that satisfies both is code the chain
rule tells you not to write, so a project should pick one.

The cost falls on alternation rather than on length, because a run of
the same method counts once. Measured against the default
`max_calls` of 5, with five stages either way:

- five consecutive `map`s: silent, counting three distinct calls.
- `map` / `filter_map` / `map` / `filter_map` / `map`: **seven**
  distinct calls, flagged.

So this rule is free where it produces a run and expensive where it
interleaves adapter kinds. In the second case the chain rule's remedy
is to name a stage — and its `CUT_HELP` declines a cut that can only
be named for the steps it performs rather than for the value it
yields. One step per adapter produces exactly those stages, so the
joint style is one the chain rule identifies as wrong while this rule
compels it.

## Configuration

None. There is one trigger and one direction. Whether a project wants
the rule at all is the `[perfectionist]` `enable` decision, not a
knob.

## Implementation notes

A `LateLintPass`, because the trigger needs types: the receiver has to
be an `Iterator`, and the liftability of each step depends on what its
result borrows from.

- `check_expr` on `ExprKind::MethodCall` whose segment names one of
  the adapters, gated by the trait it resolves to:
  `clippy_utils::is_trait_method(cx, expr, sym::Iterator)` for the
  sequential set, and a `DefPath` match for
  `rayon::iter::ParallelIterator`, which carries no diagnostic item to
  ask for instead. The adapter set decides which argument holds the
  closure, and whether the item is its only parameter.
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

### Difficulty

**Medium.** The walk and the adapter table are easy. Liftability and
position are what raise it. Liftability decides whether a suggestion
*compiles*: a borrow of the binding is what separates one that does
from one that does not, and the conservative answer has to be the one
that declines. Position decides whether a suggestion is *correct*, and
is the only place in this rule where being wrong produces a rewrite
that compiles and behaves differently — which is why this stage
exercises it on the binary adapters alone. Rayon costs a little more
again: its trait has no diagnostic item, so it is matched by path, and
a fixture for it needs a stub of the trait rather than the crate. A
first implementation may restrict itself to `Iterator`, and to steps
taking the receiver by value, and leave the rest unflagged — a real
subset rather than a token one.

## Default state

Inactive by default. Enable in `[perfectionist].enable`.

The shape it flags is idiomatic: a closure chaining two calls appears
throughout published Rust, so a rule firing on it by default would be
arguing with most of its audience on first run. It also forces the
choice described in
[Interaction with sibling rules](#interaction-with-sibling-rules), and
a rule that forces a choice should be one a project opts into rather
than one it inherits.

## Later stage: the unanchored chain

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
- **Decide what an upgrade does.** A project that enabled the rule
  would get more findings from the same configuration. Either that is
  accepted and said out loud, or the wider trigger arrives as a
  configuration value — which would be this rule's first, against a
  catalogue that prefers a rule with one direction over a knob.

The destructured item parameter that
[Exemptions](#exemptions) declines is *not* part of this: that one is
about where the chain bottoms out, and the anchor is about where it
stops at the top.
