# `map_or_usage`

**Sources:**
<https://github.com/KSXGitHub/perfectionist/issues/515> asked for a rule
banning `map_or` and `map_or_else`, naming the two pull requests that
moved a Rust codebase off them —
<https://github.com/pnpm/pnpm/pull/16686> and
<https://github.com/pnpm/pnpm/pull/16702>, the second of which rewrote
some 62 call sites and recorded the preference in that project's
`CODE_STYLE_GUIDE.md`.

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

**Prefer** a chain, which states the same steps in the order they
evaluate:

```rust
let width = terminal_width.map(|width| width.saturating_sub(2)).unwrap_or(80);
let label = parsed.map(Entry::into_name).unwrap_or_else(|error| error.to_string());
```

**or a `match`**, where the default is itself a branch:

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
is not in execution order". That lint is the reverse of this one, and
[Interaction with Clippy](#interaction-with-clippy) is about what a
consumer does with the pair.

A diff is where the cost lands hardest. The mapper is usually the part
that changed, and in `map_or` it sits behind an argument that belongs to
the other branch, so a reviewer reads the exception first and the
change second.

What the preference gives up is one call's worth of brevity: `map_or`
says in one call what the chain says in two. Conciseness is what the
Clippy lint optimises for, and this catalogue trades it for order.

## Why one rule, not two

[One rule per file, one `Config` per rule](../CLAUDE.md#one-rule-per-file-one-config-per-rule)
asks whether a file's checks have distinct triggers, disjoint
configuration and no shared diagnostic. `map_or` and `map_or_else`
share all three: one trigger (a call that takes its default before its
mapper), one diagnostic, and no configuration at all. They differ in
whether the default is a value or a closure, which decides which
remedy the help text names and nothing about whether to fire.

Two rules would therefore give a consumer two names for one preference,
and a project that wants `map_or` and not `map_or_else` can say so with
`#[allow(perfectionist::map_or_usage)]` where the second one is
written.

## The name

The anti-pattern is the call, so the name is the call's — the shape
`thiserror_usage` already wears for this catalogue's other ban.
`#[deny(perfectionist::map_or_usage)]` reads as forbidding the use of
`map_or`, and `#[allow(perfectionist::map_or_usage)]` as permitting it
at one site.

Mirroring `clippy::map_unwrap_or` is what
[Mirror the Clippy name only for a genuine refinement](./IMPLEMENTATION_CONVENTIONS.md#mirror-the-clippy-name-only-for-a-genuine-refinement)
forbids. That lint fires on the chain this rule asks for, so sharing
its name would tell a reader the opposite of what the rule does.

`map_or` in the name reaches `map_or_else` the way `map_or_else`'s own
name extends `map_or`'s, and a name spelling both out would claim no
more than this one does.

## The remedies

Which rewrite is right depends on the default, not on the receiver, so
the diagnostic names the remedy for the default it sees:

| the call                           | the remedy                               |
|------------------------------------|------------------------------------------|
| `opt.map_or(false, f)`             | `opt.is_some_and(f)`                     |
| `opt.map_or(true, f)`              | `opt.is_none_or(f)`                      |
| `res.map_or(false, f)`             | `res.is_ok_and(f)`                       |
| `opt.map_or(None, f)`              | `opt.and_then(f)`                        |
| `res.map_or(None, Some)`           | `res.ok()`                               |
| `x.map_or(Default::default(), f)`  | `x.map(f).unwrap_or_default()`           |
| `x.map_or(default, f)`             | `x.map(f).unwrap_or(default)`            |
| `x.map_or_else(fallback, f)`       | `x.map(f).unwrap_or_else(fallback)`      |

Notes on the table's edges:

- **`Result` has no `is_err_or`.** `core` gives `Result` `is_ok_and`
  and `is_err_and` and no complement of either, so
  `res.map_or(true, f)` has no short form and takes the chain.
- **An identity mapper does not survive the chain.** Rewriting
  `opt.map_or(0, |n| *n)` as `opt.map(|n| *n).unwrap_or(0)` earns
  `clippy::map_clone`, which is warn by default; `opt.copied().unwrap_or(0)`
  is the form that lands, and `cloned` is the owning counterpart. A
  mapper that hands its parameter straight back goes the same way,
  under `clippy::map_identity`.
- **A `Default::default()` default has a row of its own for a reason.**
  Swapping it where it stands gives `.unwrap_or(Default::default())`,
  which is `clippy::unwrap_or_default`, warn by default.
- **The chain does not make the default lazy.** `unwrap_or` evaluates
  its argument whether or not the receiver holds a value, exactly as
  `map_or` does, so a default that does work is still work done for
  nothing. `unwrap_or_else` and a `match` are what fix that, and the
  pnpm pull requests found a site where it mattered: a `map_or` whose
  default constructed two `Duration`s on every build, present value or
  not.
- **A `match` is the remedy whenever an arm is more than an
  expression** — several `Err` variants to tell apart, a binding to
  name, a `?` or a `return` to reach for. The chain's closures can hold
  none of those without becoming harder to read than the `match`.

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

One difference is observable, and it is the one the rule is about. With
side effects in both arguments, `x.map_or(default, f)` runs `default`
first and the mapper second; `x.map(f).unwrap_or(default)` runs them in
the order they are written. For `map_or_else` nothing changes at all:
only one of its two closures ever runs, so laziness and order are the
same before and after.

## Interaction with Clippy

Everything here was read off the toolchain this repository pins, with
`cargo clippy --explain` and fixtures under the lint's own
`#![warn(...)]`, rather than from the published lint list.

- **`clippy::map_unwrap_or` is this rule's exact reverse.** It rewrites
  `.map(f).unwrap_or(a)` and `.map(f).unwrap_or_else(g)` into `map_or`
  and `map_or_else`. It is `pedantic`, so allow by default: a consumer
  who enables `clippy::pedantic` and this rule together gets two
  warnings pointing opposite ways at one call site, and has to allow
  the Clippy lint — which is what both pnpm pull requests ended up
  doing, the second of them allowing it workspace-wide.
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
  the error. It is `nursery`, so allow by default, and a consumer who
  enables that group allows this one too.
- **Several Clippy lints at warn by default already push particular
  defaults off `map_or`, in this rule's direction.**
  `clippy::unnecessary_map_or` sends a boolean default to `is_some_and`
  / `is_none_or`, `clippy::option_map_or_none` sends `map_or(None, f)`
  to `and_then`, and `clippy::result_map_or_into_option` sends
  `map_or(None, Some)` to `ok()`. The
  `clippy::unnecessary_option_map_or_else` /
  `clippy::unnecessary_result_map_or_else` pair catches a `map_or_else`
  whose mapper is the identity.
- **The gap this rule fills is every other default.** `opt.map_or(0, plus)`
  and `res.map_or_else(recover, plus)` draw nothing from Clippy at its
  default levels.

At the defaults those warn-by-default lints do cover, a consumer sees
two warnings for one call. Firing anyway is the choice here, with the
help text naming the same short form Clippy would, so the two agree
rather than merely stack. Exempting those defaults was the
alternative, and it loses twice: the exemption would freeze Clippy's
current coverage into this rule, and it would leave a consumer who
does not run Clippy with silence where the catalogue has an opinion.

## How often is this written?

A grep over the local crate registry cache, 345 crates and 10246
source files: **549** call sites on lines that are not comments — 485
`map_or` and 64 `map_or_else` — spread across **102** of the 345
crates. Among them is `clippy_utils`, which is to say the idiom is
ordinary rather than careless.

These measurements bound the implementation:

- **Zero fully-qualified calls.** `Option::map_or(opt, 0, f)` and the
  three like it occur nowhere in the corpus, so a trigger that reads
  method calls only loses nothing measurable.
- **One third-party definition of the name**, a free function in
  `derive_more-impl`'s format-string parser, which is not a method and
  so is not a receiver this rule would ask about.

This repository's own source holds 5 `map_or` and 2 `map_or_else` call
sites, and `ui/splittable_adapter_closure_family_shapes.rs` holds nine
more.

> [!IMPORTANT]
> Read that count the opposite way to
> [`async_as_conversion`](./async-as-conversion.md)'s. It says
> the rule will fire, in published libraries and in this one, and that
> the implementing pull request owes two things before `just all`
> passes: a rewrite of perfectionist's own call sites, and the expected
> output of a fixture written for another rule.

## What to lint

A `LateLintPass` over `ExprKind::MethodCall` where both hold:

1. **The method is named `map_or` or `map_or_else`**, matched as whole
   names rather than as a prefix.
2. **The receiver is an `Option` or a `Result`** — its adjusted type,
   peeled of references, carries `sym::Option` or `sym::Result`.

The arguments decide nothing about whether to fire. They choose the
help text, per [The remedies](#the-remedies).

The diagnostic spans the method name and its arguments rather than the
whole expression, so a long receiver stays out of the report.

### Exemptions

- **A macro-synthesised call**, per
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations),
  with a `ui/map_or_usage_proc_macro.rs` fixture that fails once the
  guard is removed.
- **`map_or_default`.** It takes the mapper and nothing else, so it has
  no order to get wrong. Requirement 1's whole-name match is what keeps
  it out, and the name earns a fixture case of its own.

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

None. The preference has one direction and no threshold, and a project
that does not share it wants the rule off rather than tuned.

## Default state

Active by default. The trigger has no uncertainty in it: the shape is
read off a method name and a receiver type, and every call it finds is
the shape the rule objects to, so there is no false-positive rate to
keep in check.

`Inactive by default` was the alternative, on the grounds that the rule
puts a consumer in a fight with `clippy::map_unwrap_or`. It does not
win: that lint is allow by default, so the fight only starts for a
project that opted into `clippy::pedantic`, and such a project has
already taken on arbitrating between opinions. Shipping on is also what
`thiserror_usage` does with a comparable ban.

## Autofix

None in the first implementation, for the reason the remedies table
exists: the right rewrite depends on the default, and the two the rule
most often wants — `is_some_and` and a `match` — are not a span swap.

The chain is a span swap, and a later pull request could offer it for
`map_or_else` alone as `Applicability::MachineApplicable`: both
arguments are closures, only one of them ever runs, and the rewrite
moves them without changing what either receives. It also only ever
relaxes borrow-checking, since the two closures no longer have to
coexist — a `map_or_else` whose closures both borrow one local
mutably is `E0499`, where the chain of the same two bodies compiles.
The same swap on `map_or` is `MaybeIncorrect` at best, because it
reorders two evaluations the reader may be relying on. Either would hand a
`clippy::pedantic` consumer a diff that their own lint then objects
to, which is an argument for leaving the application to a human for as
long as the rule is new.

## Interaction with sibling rules

- **`perfectionist::splittable_adapter_closure` already reads these two
  methods**, lifting a multi-job mapper's steps into `map`
  ([`src/rules/splittable_adapter_closure/adapter.rs`](../src/rules/splittable_adapter_closure/adapter.rs)
  keys both as defaulted adapters, and its rewrite carries the default
  argument through). A `map_or` whose mapper does several jobs
  draws both diagnostics, and the two fixes compose in either order:
  splitting the closure leaves the call where it is, and moving the
  default leaves the split closure alone.
- **The two remedies have opposite costs under this catalogue's own
  rules.** The chain adds one call to a chain's spine, which
  `perfectionist::overly_long_method_chain` counts against
  `max_calls`; the `match` adds a branch, which
  `perfectionist::excessive_cognitive_complexity` scores at one plus
  the nesting depth. Neither rule is wrong to count what it counts, and
  a call site near either ceiling is a reason to pick the other
  remedy — not a reason to keep the `map_or`.

## Implementation notes

- **The receiver test** is the shape
  [`src/rules/costless_to_conversion.rs`](../src/rules/costless_to_conversion.rs)
  uses for the same pair of types: `ty_adt_def`, then
  `cx.tcx.is_diagnostic_item` against `sym::Option` and `sym::Result`.
  [`src/rules/splittable_adapter_closure/family.rs`](../src/rules/splittable_adapter_closure/family.rs)
  answers the same question for more families than this rule needs; it
  is worth reading before writing a third copy, and worth factoring
  only if a third rule wants the pair.
- **The help text** reads the first argument syntactically — a boolean
  literal, the `None` path, a `Default::default()` call — so no part of
  the rule needs a type beyond the receiver's.
- **A `Result`'s `map_or_else` keeps its error closure.** The rewrite
  moves it from the first argument of `map_or_else` to the only
  argument of `unwrap_or_else`, where it is handed the same error.
- **The fully-qualified form** `Option::map_or(opt, 0, f)` is an
  `ExprKind::Call`, which requirement 1 does not reach. Leave it; the
  corpus count found none.
- **Fixtures.** A `ui/map_or_usage.rs` covering the four methods, the
  defaults the help text keys on, `map_or_default`, and a receiver of
  some other type that defines a `map_or`-shaped method; plus the
  proc-macro regression fixture. Expect
  `ui/splittable_adapter_closure_family_shapes.stderr` to gain
  diagnostics, since every active rule runs on every fixture.

### Difficulty

**Easy.** A method name, a receiver type, and a literal's shape for the
help text. No body analysis, no source re-parsing, no configuration to
validate.
