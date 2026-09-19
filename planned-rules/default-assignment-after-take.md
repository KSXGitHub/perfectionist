# `default_assignment_after_take`

**Source:** project convention. Noticed while reviewing
[`KSXGitHub/perfectionist#472`](https://github.com/KSXGitHub/perfectionist/pull/472),
whose worked example wrote the default over a place
`std::mem::take` had already defaulted. That rule's suggested fix is
what produces the shape, so the two travel together; see
[Where the shape comes from](#where-the-shape-comes-from).

> [!IMPORTANT]
> This is a proposal with no field evidence behind it. Read
> [Evidence](#evidence) before implementing anything.

## Statement

`std::mem::take(&mut p)` replaces `p` with `Default::default()` and
hands back what was there. A statement that immediately follows it and
assigns a default to the same place writes a value the place already
holds.

**Avoid:**

```rust
let previous = std::mem::take(&mut self.pending);
self.pending = Vec::new();
```

**Prefer:**

```rust
let previous = std::mem::take(&mut self.pending);
```

## Why restrict this?

This is a stylistic preference, not a correctness issue. The second
write is wasted work, but the objection is to what it tells a reader:
an explicit reset says the place mattered and had to be cleared, so a
reader who trusts it goes looking for why `take`'s own reset was not
enough. There is no answer, and the line survives every refactor of
the code around it because nothing ever proves it unnecessary.

## Where the shape comes from

It is fallout from a specific edit, which is the argument for the
rule existing at all rather than a general appeal to tidiness.

A caller that lends a place to a function and then overwrites it
reads:

```rust
store.remember(&args.key, 1);
args.key = Key::default();
```

When the callee changes to take its parameter by value — which is
what
[`cloned_borrowed_parameter`](https://github.com/KSXGitHub/perfectionist/pull/472)
suggests — the caller cannot move out of a place behind a reference,
so the mechanical edit is to wrap the argument:

```rust
store.remember(std::mem::take(&mut args.key), 1);
args.key = Key::default();       // now writes the default twice
```

The reset was load-bearing before the edit and is not after it, and
nothing in the diff says so. A lint that catches this is a cleanup for
an edit the catalogue itself asks people to make.

## What to lint

Flag a statement `p = <default>;` when all of:

1. It is an assignment whose right-hand side is default-equivalent —
   `Default::default()`, `T::default()`, `Vec::new()` and the other
   default-equivalent constructors, `0`, `""`, `false`, `None`. This
   is exactly `clippy_utils::is_default_equivalent`, so the boundary
   is inherited rather than invented.
2. The **immediately preceding statement in the same block** contains
   `std::mem::take(&mut q)` as a subexpression, where `q` and `p` are
   the same place. The `take` is usually nested — the shape above has
   it in an argument position — so the match is over the statement's
   subexpressions, not its top level.
3. Nothing between the two reads or writes the place. Requiring
   adjacency makes this hold by construction, and is what keeps the
   rule a syntactic check on one block.

### Why `mem::replace` is not covered

`std::mem::replace(&mut p, <default>)` followed by the same assignment
is the same redundancy. It is left out because
`clippy::mem_replace_with_default` (`style`, warn by default) already
rewrites that call to `mem::take`, so a project running Clippy at all
reaches this rule's shape first. Covering both would also make the
rule's name claim more than it checks.

## Examples

**Avoid:** the reset restores a value the `take` already restored.

```rust
fn drain(&mut self) -> Vec<Job> {
    let jobs = std::mem::take(&mut self.queue);
    self.queue = Vec::new();
    jobs
}
```

**Prefer:**

```rust
fn drain(&mut self) -> Vec<Job> {
    std::mem::take(&mut self.queue)
}
```

**Not flagged:** pnpm's own caller, the one that prompted all this.
It fails twice over — the assignment is a computed value rather than a
default, and a statement sits between the two anyway.

```rust
let plan = PackageSpecifierPlan::parse(std::mem::take(&mut args.package_names))?;
check_specifier_combination(args, &plan)?;
args.package_names = plan.node_packages;
```

**Not flagged:** a statement sits between the two, so the rule cannot
see that the place was untouched.

```rust
let jobs = std::mem::take(&mut self.queue);
self.notify(&jobs);
self.queue = Vec::new();
```

**Not flagged:** a different place.

```rust
let jobs = std::mem::take(&mut self.queue);
self.staged = Vec::new();
```

## An effectful `Default` is out of scope

`std::mem::take(dest)` is defined as `replace(dest, T::default())`, so
it calls `Default::default()` exactly once. The assignment after it
calls it a second time, and deleting that assignment takes the count
from two to one. That is the whole of what this rule's suggestion
changes.

For a `Default` that yields a constant, the two calls are
indistinguishable and the second write is dead. For one that observes
or mutates something, they are not:

```rust
static NEXT: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, PartialEq)]
struct Ticket(u32);
impl Default for Ticket {
    fn default() -> Self { Ticket(NEXT.fetch_add(1, Ordering::Relaxed)) }
}
```

`mem::take(&mut t)` leaves `Ticket(0)`, and the `t = Ticket::default()`
after it leaves `Ticket(1)`. A derived `Default` has no such
behaviour, and the same program leaves `Key("")` either way.

**So the rule fires only where the `Default` is known to be pure, and
stays silent otherwise.** Silence is right twice over: the suggestion
would be wrong, and the defect in such code is the effectful `Default`
itself rather than the assignment that follows it — a different
anti-pattern, for a different rule.

### Knowing that a `Default` is pure

Three-valued, and only the first fires:

- **Known pure** — a standard-library type whose default is trivial
  (`Vec`, `String`, `Option`, the integers, …), or a
  `#[derive(Default)]` impl every one of whose field types is itself
  known pure, checked transitively with a cycle guard.
- **Known effectful** — what a rule about effectful `Default` impls
  would need. This rule does not.
- **Unknown** — everything else, including a hand-written impl that
  happens to be pure.

Under-firing on a hand-written but pure `Default` is the accepted
cost. There is no predicate in `clippy_utils` to borrow for this: its
`eager_or_lazy` module classifies a call it cannot resolve as
*expensive*, which is the right default for the lints it serves and
useless as a purity test here.

### Why Clippy's `mem::take` rewrite is not a precedent

`clippy::mem_replace_with_default` rewrites
`mem::replace(&mut x, T::default())` to `mem::take(&mut x)`, is
warn-by-default, and fires whatever `T`'s `Default` does. It is not a
precedent for firing here regardless of purity, because the two
rewrites are not alike. Clippy substitutes one `Default::default()`
call for one other — `mem::take` *is* that `replace` — so the count
is unchanged and nothing about purity is assumed. This rule's
suggestion removes a call. Only the second needs the condition above.

## Evidence

There is none yet, and that is the second thing to weigh. Every other
rule in this catalogue was distilled from a real codebase or a style
guide; this one was distilled from an example in a planning file. The
shape is a plausible consequence of a specific refactor rather than
something observed in the wild. Every `mem::take` and `mem::replace`
call site in this repository was read, and none is followed by a
default assignment; pnpm's own `mem::take`, the one that prompted the
review, is followed by a computed assignment rather than a default.

So the rule is cheap and decidable, but unproven. It may be worth
holding until the pattern turns up in a real diff.

## Implementation notes

`LateLintPass::check_block`, walking consecutive statement pairs. Two
of the three pieces come from `clippy_utils`: `is_default_equivalent`
for the right-hand side, and `SpanlessEq` for comparing the assigned
place with the one the `take` borrows.

The third is the purity classification under
[Knowing that a `Default` is pure](#knowing-that-a-default-is-pure),
which has nothing to borrow and is where the work is. It belongs in a
crate-internal module rather than inside the rule, since a rule about
effectful `Default` impls would want the same classification read the
other way round.

The suggestion is to delete the statement, which is a span removal
rather than a rewrite, and `MachineApplicable` once purity is
established.

### Difficulty

**Easy for the trigger, medium for the guard.** Two adjacent
statements in one block, no configuration, no cross-body analysis and
no parsing — but the purity classification is a recursive walk over
field types with a cycle guard, and getting its conservatism right is
most of the rule.

## Configuration

None. There is no direction to choose and no threshold to set.

## Default state

Active by default. The trigger is syntactic and narrow, the purity
guard keeps it off every case where the suggestion would be wrong, and
nothing about either varies per project.

## Interaction with clippy and sibling rules

- **`clippy::mem_replace_with_default`** (`style`, warn by default)
  rewrites `mem::replace(&mut x, T::default())` to `mem::take`. It is
  upstream of this rule rather than overlapping it: it produces the
  `take` this rule then reads. Its rewrite is safe whatever `T`'s
  `Default` does, for the reason under
  [Why Clippy's `mem::take` rewrite is not a precedent](#why-clippys-memtake-rewrite-is-not-a-precedent).
  See also
  [Why `mem::replace` is not covered](#why-memreplace-is-not-covered).
- **`clippy::field_reassign_with_default`** (`style`) flags
  `let mut x = T::default(); x.field = …;` and asks for a struct
  literal. Different statement pair, different remedy.
- **`clippy::default_trait_access`** (`pedantic`) is about spelling
  `Default::default()` where `T::default()` reads better. Orthogonal.
- **rustc's `unused_assignments`** catches the dead store when the
  place is a **local**, so the rule should stay silent there and leave
  it to the compiler. It does not fire when the place is a field
  reached through a reference, because the write is observable by the
  caller — which is the case this rule is for.
- **A rule about effectful `Default` impls** would be this one's
  complement, reading the same classification the other way round: a
  `Default` that observes or mutates something is a hazard wherever it
  is reached for a placeholder, which is `mem::take`,
  `unwrap_or_default` and `..Default::default()` alike. No such rule
  is filed. Where one exists, this rule's silence on
  [the effectful case](#an-effectful-default-is-out-of-scope) becomes
  a deferral to it rather than a gap.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
