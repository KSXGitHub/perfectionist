# `map_or_usage` and `map_or_else_usage`

**Sources:**
<https://github.com/KSXGitHub/perfectionist/issues/515> asked for a rule
banning `map_or` and `map_or_else`, naming the two pull requests that
moved a Rust codebase off them —
<https://github.com/pnpm/pnpm/pull/16686> and
<https://github.com/pnpm/pnpm/pull/16702>, the second of which rewrote
some 62 call sites and recorded the preference in that project's
`CODE_STYLE_GUIDE.md`. The issue asks for one rule;
[Two rules, one per method](#two-rules-one-per-method) is why this file
plans two.

## Statement

`Option::map_or`, `Option::map_or_else`, `Result::map_or` and
`Result::map_or_else` take the default first and the mapper second. The
reader therefore meets the value the receiver does *not* hold before the
one it does, and the two arguments stand in the reverse of the order
they run in.

**Avoid:**

```rust
let width = terminal_width.map_or(80, |width| width.saturating_sub(2));
let label = parsed.map_or_else(|error| error.to_string(), Entry::into_name);
```

**Prefer:** a chain, which states the steps in the order they evaluate

```rust
let width = terminal_width.map(|width| width.saturating_sub(2)).unwrap_or(80);
let label = parsed.map(Entry::into_name).unwrap_or_else(|error| error.to_string());
```

**Prefer:** a `match`, where the default is itself a branch

```rust
let label = match parsed {
    Ok(entry) => entry.into_name(),
    Err(Error::Missing) => String::new(),
    Err(error) => error.to_string(),
};
```

## Why restrict this?

This is a stylistic preference, not a correctness issue. Each of the
four methods returns exactly what the chain returns, which
[Equivalence of the remedies](#equivalence-of-the-remedies) measures,
and the `match` is that same expression written out.

The preference is that a call should read in the order it runs. A reader
who reaches `terminal_width.map_or(` has been handed a receiver and must
now hold `80` — a number that means nothing yet — until the mapper
arrives to explain it. The chain hands over the receiver, then what
happens to it, then what stands in when there is nothing to happen to,
which is the order the machine takes them in.

Clippy records the same observation against the shape it suggests, under
`clippy::map_unwrap_or`'s *Known problems*: "The order of the arguments
is not in execution order". That lint is the reverse of these two, and
[Interaction with Clippy](#interaction-with-clippy) is about what a
consumer does with the pair.

A diff is where the cost lands hardest. The mapper is usually the part
that changed, and under a default-first call it sits behind an argument
that belongs to the other branch, so a reviewer reads the exception
first and the change second.

What the preference gives up is one call's worth of brevity: `map_or`
says in one call what the chain says in two. Conciseness is what the
Clippy lint optimises for, and this catalogue trades it for order.

## Two rules, one per method

`map_or_usage` fires on `Option::map_or` and `Result::map_or`.
`map_or_else_usage` fires on `Option::map_or_else` and
`Result::map_or_else`. One rationale covers both, which is an argument
for one file rather than for one rule: what the two methods share is
the sentence under [Statement](#statement), and everything an
implementer and a consumer have to decide differs. Measured over the
corpus [How often is this written?](#how-often-is-this-written)
describes:

| | `map_or` | `map_or_else` |
|---------------------------------|--------------|--------------|
| call sites                      | 485          | 64           |
| a boolean literal as the default| 248 (51%)    | 0            |
| the default spans source lines  | 4 (1%)       | 16 (25%)     |
| Clippy at its default levels    | redirects the boolean half | silent, bar the degenerate mapper |
| the short remedies              | `is_some_and`, `is_none_or`, `is_ok_and`, `and_then`, `ok`, `unwrap_or_default` | none apply |
| an MSRV floor on the remedy     | 1.70 and 1.82 for the predicates | none |
| taking the chain by hand        | reorders the default against the mapper | changes nothing observable |

The call counts and the line-spanning figures point opposite ways:
`map_or` is the common call whose default is a token, and `map_or_else`
is the rare one whose default is a body that a quarter of the time
pushes the mapper onto another line. The reversal costs least where it
happens most, so the
two rules earn different arguments for being on, and a project may well
want to disable the frequent, milder one without losing the other.

[One rule per file, one `Config` per rule](../CLAUDE.md#one-rule-per-file-one-config-per-rule)
asks whether the checks have distinct triggers, disjoint configuration
and no shared diagnostic. The triggers are disjoint by method name, the
configuration is empty on both sides, and the diagnostics differ in
every part a reader acts on: which remedies the help names, whether an
MSRV bears on them, and what taking one costs at that site. A single
rule would also have to carry a name claiming one method while
firing on another, which
[Do not over-claim in the name](./IMPLEMENTATION_CONVENTIONS.md#do-not-over-claim-in-the-name)
rules out.

What the two share is implementation rather than policy: one receiver
test and one span, which
[Implementation notes](#implementation-notes) factors into a module
both rules read.

## The names

The anti-pattern is the call, so each name is its call's — the shape
`thiserror_usage` already has as this catalogue's other ban.
`#[deny(perfectionist::map_or_else_usage)]` reads as forbidding the use
of `map_or_else`, and `#[allow(perfectionist::map_or_usage)]` as
permitting a `map_or` at one site. Neither name reaches the other's
method, which is the point of spelling both out.

Mirroring `clippy::map_unwrap_or` is what
[Mirror the Clippy name only for a genuine refinement](./IMPLEMENTATION_CONVENTIONS.md#mirror-the-clippy-name-only-for-a-genuine-refinement)
forbids. That lint fires on the chain these rules ask for, so sharing
its name would tell a reader the opposite of what they do.

## The remedies

Which rewrite is right depends on the default, so each diagnostic
names, in its `help:` text, the remedies that fit the default it sees.
It names them; it never emits one as a replacement, per
[Autofix](#autofix).

`map_or_usage`:

| the call                           | the remedy                               |
|------------------------------------|------------------------------------------|
| `opt.map_or(false, f)`             | `opt.is_some_and(f)`                     |
| `opt.map_or(true, f)`              | `opt.is_none_or(f)`                      |
| `res.map_or(false, f)`             | `res.is_ok_and(f)`                       |
| `opt.map_or(None, f)`              | `opt.and_then(f)`                        |
| `res.map_or(None, Some)`           | `res.ok()`                               |
| `x.map_or(Default::default(), f)`  | `x.map(f).unwrap_or_default()`           |
| `x.map_or(default, f)`             | `x.map(f).unwrap_or(default)`            |

`map_or_else_usage`: `x.map_or_else(fallback, f)` becomes
`x.map(f).unwrap_or_else(fallback)`, or a `match`. The predicate
methods take a value's test rather than a closure's result, so none of
them applies to a call whose default is already a closure.

Notes on the tables' edges:

- **The predicates have MSRV floors.** `is_some_and`, `is_ok_and` and
  `is_err_and` stabilised in 1.70 and `is_none_or` in 1.82, so a help
  text naming one names a method some consumers cannot call. Naming the
  chain alongside it keeps the advice valid at every version, which is
  also why neither rule reads an MSRV; see
  [Configuration](#configuration).
- **`Result` has no `is_err_or`.** `core` gives `Result` `is_ok_and`
  and `is_err_and` and no complement of either, so
  `res.map_or(true, f)` has no short form and takes the chain.
- **An identity mapper does not survive the chain.** Rewriting
  `opt.map_or(0, |n| *n)` as `opt.map(|n| *n).unwrap_or(0)` earns
  `clippy::map_clone`, which is warn by default;
  `opt.copied().unwrap_or(0)` is the form that lands, and `cloned` is
  the owning counterpart. A mapper that hands its parameter straight
  back goes the same way, under `clippy::map_identity`.
- **A `Default::default()` default has a row of its own for a reason.**
  Swapping it where it stands gives `.unwrap_or(Default::default())`,
  which is `clippy::unwrap_or_default`, warn by default.
- **Neither chain makes the default lazy.** `unwrap_or` evaluates its
  argument whether or not the receiver holds a value, exactly as
  `map_or` does, so a default that does work is still work done for
  nothing. `unwrap_or_else` and a `match` are what fix that, and the
  pnpm pull requests found a site where it mattered: a `map_or` whose
  default constructed two `Duration`s on every build, present value or
  not.
- **A `match` is the remedy whenever an arm is more than an
  expression** — several `Err` variants to tell apart, a binding to
  name, a `?` or a `return` to reach for. A closure can hold none of
  those without becoming harder to read than the `match`.

### Equivalence of the remedies

Measured on the pinned toolchain, over `Some(2)` / `None` and
`Ok(2)` / `Err("boom")`, with `|n| n + 1` as the mapper and a default
of `9` (`|e| e.len() as i32` for the `Result` closures):

| the call                   | `map_or*` | the chain |
|----------------------------|-----------|-----------|
| `Option::map_or`           | `3` / `9` | `3` / `9` |
| `Option::map_or_else`      | `3` / `9` | `3` / `9` |
| `Result::map_or`           | `3` / `9` | `3` / `9` |
| `Result::map_or_else`      | `3` / `4` | `3` / `4` |

`Result::map_or_else` hands its first closure the error, and
`Result::unwrap_or_else` hands its only closure the error, so the
rewrite moves that closure without changing what it receives.

One difference is observable, and it is the one these rules are about.
With side effects in both arguments, `x.map_or(default, f)` runs
`default` first and the mapper second; `x.map(f).unwrap_or(default)`
runs them in the order they are written. For `map_or_else` nothing
changes at all: only one of its two closures ever runs, so laziness and
order are the same before and after.

The chain also only ever relaxes borrow-checking, since the two
arguments no longer have to coexist: a `map_or_else` whose closures
both borrow one local mutably is `E0499`, where the chain of the same
two bodies compiles. So a reader told to take the chain is never told
to write something that will not build — which is a property of the
rewrite, not a reason to apply it for them.

## Interaction with Clippy

Everything here was read off the toolchain this repository pins, with
`cargo clippy --explain` and fixtures under the lint's own
`#![warn(...)]`, rather than from the published lint list.

- **`clippy::map_unwrap_or` is the exact reverse of both rules.** It
  rewrites `.map(f).unwrap_or(a)` and `.map(f).unwrap_or_else(g)` into
  `map_or` and `map_or_else`. It is `pedantic`, so allow by default: a
  consumer who enables `clippy::pedantic` alongside either rule gets
  two warnings pointing opposite ways at one call site, and has to
  allow the Clippy lint — which is what both pnpm pull requests ended
  up doing, the second of them allowing it workspace-wide.
- **Its own documentation understates it.** The text lists
  `option.map(_).unwrap_or(_)`, `option.map(_).unwrap_or_else(_)` and
  `result.map(_).unwrap_or_else(_)`, omitting
  `result.map(_).unwrap_or(_)` — on which the lint fires all the same,
  reporting "called `map(<f>).unwrap_or(<a>)` on a `Result` value". Do
  not plan around the omission.
- **`clippy::option_if_let_else` is the reverse of the `match`
  remedy.** It rewrites an `if let` or `match` over an `Option` into
  `map_or` / `map_or_else`, and does the same for a `Result` whose
  `Err` arm ignores the error; it stays silent where an `Err` arm uses
  the error. It is `nursery`, so allow by default, and a project that
  enables `clippy::nursery` has to allow `clippy::option_if_let_else`
  for the `match` remedy to stand.
- **`map_or_usage` overlaps Clippy's warn-by-default lints and
  `map_or_else_usage` barely does.**
  `clippy::unnecessary_map_or` sends a boolean default to `is_some_and`
  / `is_none_or`, `clippy::option_map_or_none` sends `map_or(None, f)`
  to `and_then`, and `clippy::result_map_or_into_option` sends
  `map_or(None, Some)` to `ok()` — half of all `map_or` calls in the
  corpus and none of the `map_or_else` ones. What touches
  `map_or_else` is the
  `clippy::unnecessary_option_map_or_else` /
  `clippy::unnecessary_result_map_or_else` pair, which catches a
  mapper that is the identity.
- **`clippy::unnecessary_map_or` is MSRV-gated as a whole.** Under
  `rust-version = "1.80"` it reports neither boolean shape, the
  `is_some_and` one included, although that method stabilised in 1.70.
  So in a crate pinned below 1.82 the overlap vanishes and
  `map_or_usage` is the only lint with anything to say.
- **The gap both rules fill is every other default.**
  `opt.map_or(0, plus)` and `res.map_or_else(recover, plus)` draw
  nothing from Clippy at its default levels.

Where the overlap does bite, firing anyway is the choice here, with the
help text naming the same short form Clippy would, so the two agree
rather than merely stack. Exempting the shapes Clippy covers was the
alternative, and it loses twice: the exemption would freeze Clippy's
current coverage into `map_or_usage`, and it would leave a consumer who
does not run Clippy — or who pins an MSRV below 1.82 — with silence
where the catalogue has an opinion.

## How often is this written?

A grep over the local crate registry cache, 345 crates and 10246
source files: **549** call sites on lines that are not comments — 485
`map_or` and 64 `map_or_else` — spread across **102** of the 345
crates. Among them is `clippy_utils`, which is to say the idiom is
ordinary rather than careless.

A scan of each call's first argument put the shape figures in
[Two rules, one per method](#two-rules-one-per-method). Two of those
figures are evidence about Clippy rather than about the idiom:
every `map_or` whose default is `None` has already been rewritten
somewhere in the corpus — there are none left — which is what a
warn-by-default lint does over time, while the boolean default survives
in 248 calls because the lint covering it is both newer and MSRV-gated.

These measurements bound the implementation:

- **Zero fully-qualified calls.** `Option::map_or(opt, 0, f)` and the
  three like it occur nowhere in the corpus, so a trigger that reads
  method calls only loses nothing measurable.
- **One third-party definition of either name**, a free function in
  `derive_more-impl`'s format-string parser, which is not a method and
  so is not a receiver these rules would ask about.

This repository's own source holds 5 `map_or` and 2 `map_or_else` call
sites, and `ui/splittable_adapter_closure_family_shapes.rs` holds nine
more.

> [!IMPORTANT]
> Read that count the opposite way to
> [`async_as_conversion`](./async-as-conversion.md)'s. It says the
> rules will fire, in published libraries and in this one, and that the
> implementing pull request owes two things before `just all` passes: a
> rewrite of perfectionist's own call sites, and the expected output of
> a fixture written for another rule.

## What to lint

One `LateLintPass` per rule, over `ExprKind::MethodCall`, where both
hold:

1. **The method is the rule's**, matched as a whole name rather than as
   a prefix: `map_or` for `map_or_usage`, `map_or_else` for
   `map_or_else_usage`.
2. **The receiver is an `Option` or a `Result`** — its adjusted type,
   peeled of references, carries `sym::Option` or `sym::Result`.

The arguments decide nothing about whether to fire. They choose the
help text, per [The remedies](#the-remedies).

Each diagnostic spans the method name and its arguments rather than the
whole expression, so a long receiver stays out of the report.

### Exemptions

- **A macro-synthesised call**, per
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations),
  with a `ui/<rule>_proc_macro.rs` fixture apiece that fails once the
  guard is removed.
- **`map_or_default`.** It takes the mapper and nothing else, so it has
  no order to get wrong. Requirement 1's whole-name match is what keeps
  it out of `map_or_usage`, and the name earns a fixture case of its
  own.

There is **no test-code exemption**. The argument is about what a
reader does with the call, and a test's reader is the same reader; the
remedies cost a test what they cost anything else. A project that
disagrees reaches for `#[allow(perfectionist::map_or_usage)]` on the
test module.

A **const context** needs no exemption either. `Option`'s whole family
is gated on `const_option_ops` and `Result`'s on
`const_result_trait_fn`, so a `const fn` that can call `map_or` can
call `map` and `unwrap_or` under the same feature.

## Configuration

None, on either rule. The preference has one direction and no
threshold, and a project that does not share it wants the rule off
rather than tuned.

An `msrv` knob is the one knob with a case, since `map_or_usage`'s
short remedies are newer than its trigger. It is not worth taking:
reading the consumer's `rust-version` would decide only *which* remedy
the help names, and naming the chain beside the predicate answers that
for every version at no cost. A consumer whose MSRV predates the
predicates still gets advice they can take.

## Default state

Active by default, both. Neither trigger has any uncertainty in it: the
shape is read off a method name and a receiver type, and every call
either finds is the shape it objects to, so there is no false-positive
rate to keep in check.

The arguments differ in the way the split predicts.
`map_or_else_usage` fires rarely, on the calls where the reversal
actually separates the mapper from its receiver, and its remedy is
mechanical. `map_or_usage` fires often, on calls whose default is a
token, and overlaps Clippy at half of them — the weaker case of the
two, and still the catalogue's opinion to state, since the overlap
disappears for a consumer who either pins an MSRV below 1.82 or does
not run Clippy.

`Inactive by default` was the alternative for `map_or_usage`, on the
grounds that it puts a consumer in a fight with
`clippy::map_unwrap_or`. It does not win: that lint is allow by
default, so the fight only starts for a project that opted into
`clippy::pedantic`, and such a project has already taken on
arbitrating between opinions. Shipping on is also what
`thiserror_usage` does with a comparable ban.

## Autofix

None, on either rule, and no structured suggestion either: the help
text names the candidate remedies in prose and the diagnostic carries
no replacement span. `cargo dylint --fix` applies a
`MachineApplicable` suggestion, so withholding the span is what keeps
the choice with the author; an `Applicability::Unspecified` suggestion
would not be applied but would still render one rewrite as *the*
answer, which is the same claim in quieter type.

The issue this file comes from puts the reason in one clause: the right
fix is a chain or a `match` "depending on the situation". A lint can
read the arguments and not the situation. Whether a `match` beats the
chain turns on what the arms would hold, whether the default ought to
stop being eager, how far the mapper has drifted from its receiver, and
how the call reads against its neighbours — judgement the author has
and the rule does not.

Encoding criteria for that choice is the alternative, and it costs more
than it buys: a rule whose whole trigger is a method name and a
receiver type would grow a second, larger half that decides between two
restructurings, with its own conditionals to maintain and its own way
of being wrong. Being wrong there is worse than being silent, because a
fix the author would not have chosen arrives already applied.

`map_or_else_usage`'s chain is the one rewrite that is mechanically
safe — [Equivalence of the remedies](#equivalence-of-the-remedies)
measures that it preserves the value, the laziness and the order, and
only relaxes borrow-checking. It is still not offered. Mechanically
safe is not the same as better reading, and a `match` is often the
better reading at exactly the sites where `map_or_else_usage` has the
most to say: the quarter of them whose default spans lines.

The boolean default is the one shape with a single right answer
(`is_some_and`, `is_none_or`, `is_ok_and`), and it needs no suggestion
from this catalogue: `clippy::unnecessary_map_or` already carries an
applicable one wherever the consumer's MSRV allows it, and naming the
method in the help covers the consumers it skips.

## Interaction with sibling rules

- **`perfectionist::splittable_adapter_closure` already reads both
  methods**, lifting a multi-job mapper's steps into `map`
  ([`src/rules/splittable_adapter_closure/adapter.rs`](../src/rules/splittable_adapter_closure/adapter.rs)
  keys both as defaulted adapters, and its rewrite carries the default
  argument through). A call whose mapper does several jobs draws two
  diagnostics, and the two fixes compose in either order: splitting the
  closure leaves the call where it is, and moving the default leaves
  the split closure alone.
- **The two remedies have opposite costs under this catalogue's own
  rules.** The chain adds one call to a chain's spine, which
  `perfectionist::overly_long_method_chain` counts against
  `max_calls`; the `match` adds a branch, which
  `perfectionist::excessive_cognitive_complexity` scores at one plus
  the nesting depth. Neither rule is wrong to count what it counts, and
  a call site near either ceiling is a reason to pick the other
  remedy — not a reason to keep the default-first call.

## Implementation notes

- **One module holds what both rules need**, per
  [One rule per file, one `Config` per rule](../CLAUDE.md#one-rule-per-file-one-config-per-rule):
  the receiver test, the diagnostic span, and the help-text builder
  that reads the first argument. Each rule's own file keeps its
  `declare_tool_lint!` block, its `Register` impl and its pass.
- **The receiver test** is the shape
  [`src/rules/costless_to_conversion.rs`](../src/rules/costless_to_conversion.rs)
  uses for the same pair of types: `ty_adt_def`, then
  `cx.tcx.is_diagnostic_item` against `sym::Option` and `sym::Result`.
  [`src/rules/splittable_adapter_closure/family.rs`](../src/rules/splittable_adapter_closure/family.rs)
  answers the same question for more families than these rules need; it
  is worth reading before writing a third copy.
- **The help text** reads the first argument syntactically — a boolean
  literal, the `None` path, a `Default::default()` call — so no part of
  either rule needs a type beyond the receiver's.
- **Emit with `span_lint_hir_and_then` and a `help` note**, not
  `span_lint_and_sugg`: the remedies are named rather than rendered,
  per [Autofix](#autofix), and the `hir` form is what carries the
  `HirId` the lint level and the proc-macro guard are read from.
- **A `Result`'s `map_or_else` keeps its error closure.** The rewrite
  moves it from the first argument of `map_or_else` to the only
  argument of `unwrap_or_else`, where it is handed the same error.
- **The fully-qualified form** `Option::map_or(opt, 0, f)` is an
  `ExprKind::Call`, which requirement 1 does not reach. Leave it; the
  corpus count found none.
- **Fixtures.** A `ui/map_or_usage.rs` covering both receivers, the
  defaults the help text keys on, `map_or_default`, and a receiver of
  some other type that defines a `map_or`-shaped method; a
  `ui/map_or_else_usage.rs` covering both receivers and the error
  closure; and a proc-macro regression fixture per rule. Expect
  `ui/splittable_adapter_closure_family_shapes.stderr` to gain
  diagnostics, since every active rule runs on every fixture.

### Difficulty

**Easy** for either rule: a method name, a receiver type, and a
literal's shape for the help text. No body analysis, no source
re-parsing, no configuration to validate. `map_or_else_usage` costs
almost nothing once `map_or_usage` exists, since the shared module
is what either one of them is mostly made of.
