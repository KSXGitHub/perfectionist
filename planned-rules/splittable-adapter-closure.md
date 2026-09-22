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
several operations makes one adapter do all of them, and the stages
that would have been visible in the pipeline are hidden inside it.

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

Flag a closure passed to one of the adapters below whose body is a
chain of **two or more** applications of the closure's parameter.

Suggest one adapter per application. Every application but the last
becomes a `map`; the last keeps the original adapter, because that is
the one whose kind the pipeline depends on:

```rust
.adapter(|binding| third(second(first(binding))))
// becomes
.map(|binding| first(binding))
.map(second)
.adapter(third)
```

A body with only **one** application is not this rule's: there is
nothing to split, and `.map(|text| text.trim())` is already one
adapter doing one thing. Reducing it to `.map(str::trim)` is
`clippy::redundant_closure_for_method_calls`.

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
**binary** closure: the item is the second parameter, the result is an
accumulator rather than an item, and the item side splits the same
way. They are in scope, and the trigger has to find the item parameter
rather than assume the only one:

```rust
// Avoid
let total = lines.fold(0, |total, line| total + line.trim().len());
// Prefer
let total = lines.map(str::trim).map(str::len).fold(0, |total, len| total + len);
```

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

### What Clippy already says

Measured on Clippy 1.94, at default levels Clippy says nothing about
any of this. Two of its lints meet this rule at the edges:

- `clippy::redundant_closure` (`style`, warn-by-default) reduces
  `|text| trim(text)` to `trim`, completing a split this rule made.
- `clippy::redundant_closure_for_method_calls` (`pedantic`,
  allow-by-default) does the same for a method call, suggesting
  `T0::bar` for `|value| value.bar()`. It is also what covers the
  single-application closures this rule leaves alone.

Nothing in Clippy splits a chained closure, which is the part this
rule is for.

### Exemptions

Do *not* flag:

- A closure whose body is not a chain of applications of its
  parameter — a block with statements, a `match`, a `?`, an operator
  expression, or a call that uses the parameter more than once.
- A step whose result borrows from the binding, per
  [When a step can be lifted](#when-a-step-can-be-lifted).
- A closure produced by a macro expansion, where the suggestion would
  be written into the macro body.
- An adapter that is not `Iterator`'s. `Option` and `Result` have
  `map` and `and_then` of their own, and this rule does not reach
  them.

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
is to name a stage — and its own help text says that if the only name
that fits describes the steps rather than the value, the cut is in the
wrong place. One-operation-per-adapter produces exactly those stages,
so the joint style is one the chain rule identifies as wrong while
this rule compels it.

## Configuration

None. There is one trigger and one direction. Whether a project wants
the rule at all is the `[perfectionist]` `enable` decision, not a
knob.

## Implementation notes

A `LateLintPass`, because the trigger needs types: the receiver has to
be an `Iterator`, and the liftability of each step depends on what its
result borrows from.

- `check_expr` on `ExprKind::MethodCall` whose segment names one of
  the adapters, gated by `clippy_utils::is_trait_method(cx, expr,
  sym::Iterator)`. The adapter set decides which argument holds the
  closure, and whether the item is its only parameter.
- Walk the closure body as a chain: a `MethodCall` whose receiver is
  the next link, or a `Call` whose sole argument is. The chain ends at
  the closure's parameter; anything else ends the walk without a
  finding.
- Two or more links is the trigger. One link is
  `redundant_closure_for_method_calls`' business, not this rule's.
- For each link but the last, decide liftability from the result type:
  decline where it carries a lifetime derived from the receiver.
  Taking the receiver by value is the easy sufficient condition.
- Suppress proc-macro-synthesised nodes per
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).
  The diagnostic span is the adapter's method segment, which a derive
  can give a user-source span, so the guard is needed and a
  `ui/<rule>_proc_macro.rs` fixture should prove it.

### Difficulty

**Medium.** The walk and the adapter table are easy. What raises it is
the liftability test: a borrow of the binding is what separates a
suggestion that compiles from one that does not, and the conservative
answer has to be the one that declines. A first implementation may
restrict itself to steps taking the receiver by value and leave the
rest unflagged, which is a real subset rather than a token one.

## Default state

Inactive by default. Enable in `[perfectionist].enable`.

The shape it flags is idiomatic: a closure chaining two calls appears
throughout published Rust, so a rule firing on it by default would be
arguing with most of its audience on first run. It also asks a project
to choose between this and
`perfectionist::overly_long_method_chain`, and a rule that forces that
choice should be one a project opts into rather than one it inherits.
