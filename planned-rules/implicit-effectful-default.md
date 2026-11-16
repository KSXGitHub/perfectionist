# `implicit_effectful_default`

**Source:** project convention. A call that constructs a
`Default::default()` the programmer never wrote, for a type whose
`Default` does something observable. The effect is invisible at the
call site, which is the whole complaint.

> [!IMPORTANT]
> Deciding whether a `Default` is observationally effectful is a
> **heuristic**, not an analysis, for the reason set out in
> [Observational referential transparency](./IMPLEMENTATION_CONVENTIONS.md#observational-referential-transparency).
> The rule may only claim what a denylist can support, and its
> rustdoc has to say so.

## Statement

Several standard-library APIs call `Default::default()` on the
caller's behalf. Nobody writes `Default::default()` at those call
sites; it happens because the API needs a value and the type said it
had one.

When that `Default` is observationally referentially transparent — it
yields the same value every time, under
[the definition](./IMPLEMENTATION_CONVENTIONS.md#observational-referential-transparency)
— the arrangement is invisible and fine. When it is not, the call
mints a fresh identifier, reads a clock, or advances a counter, and
the source gives no sign of it.

```rust
struct SessionId(Uuid);
impl Default for SessionId {
    fn default() -> Self { SessionId(Uuid::new_v4()) }
}

// Reads as "hand me the old session id". Also mints a new one.
let previous = std::mem::take(&mut self.session);
```

## Why restrict this?

This is a stylistic preference, not a correctness issue: the effect
is legal, the type opted into it, and the program may well want it.
The objection is that the call site does not say so. A reader
auditing where session identifiers come from greps for `Uuid::new_v4`
and `SessionId::default`, and this line answers to neither.

The strength of the objection varies by call, which is why
[Configuration](#configuration) tiers them rather than firing on all
of them equally.

### Where the value is never read

`mem::take`, `Cell::take` and `RefCell::take` construct a default
**as a placeholder**. Rust has no holes, so something must go in the
gap the taken value left; the caller wants the old value and never
looks at the new one.

An observable effect here is unrequested in the strongest sense. The
program pays for it, something in the world changes, and the value it
produced is dead on arrival. Measured with a counting `Default`, all
three run it on every call:

```
mem::take(&mut a)      Default::default() ran 1 time(s)
Cell::take()           Default::default() ran 1 time(s)
RefCell::take()        Default::default() ran 1 time(s)
```

### Where the value is what you asked for

`unwrap_or_default`, `or_default` and `get_or_insert_default`
construct a default **as a fallback**. They are lazy — the call
happens only when there was nothing there — and the value they build
is the one the caller goes on to use:

```
Some(_).unwrap_or_default()       Default::default() ran 0 time(s)
None.unwrap_or_default()          Default::default() ran 1 time(s)
entry(present).or_default()       Default::default() ran 0 time(s)
entry(vacant).or_default()        Default::default() ran 1 time(s)
Option::get_or_insert_default()   Default::default() ran 1 time(s)
```

A fresh identifier from `entry(k).or_default()` may be exactly right.
The complaint is only that the source does not show it, which is
weaker than the placeholder case and is why this tier is off by
default.

## What to lint

Flag a call when both:

1. It is one of the recognised constructors in
   [Configuration](#configuration) — an API that invokes
   `Default::default()` on the caller's behalf, where the call does
   not appear in the source.
2. The type it constructs is **known non-ORT**, per
   [the denylist](./IMPLEMENTATION_CONVENTIONS.md#the-two-polarities).
   Unknown is not non-ORT; silence on unknown.

### `..Default::default()` is not in scope

The struct-update form looks like it belongs and does not, because
clause 1 fails: the programmer *wrote* `Default::default()`. It is
visible, and the reader can see what it costs.

There is a real complaint about it — it builds the whole default and
discards whatever the literal overrides, measured at one wasted
`SessionId::default()` for
`Config { id: SessionId(7), ..Default::default() }` — but that is a
waste complaint rather than an invisibility one, and it wants its
own rule.

### Generic code and derives are not in scope either

A generic `fn f<T: Default>()` calling `T::default()`, and
`#[derive(Default)]` on a struct with an effectful field, both
construct defaults the *instantiating* code never wrote. They fit the
spirit and not the trigger: the call sites are in the generic body
and in the derive expansion, neither of which is where a reader would
act. A definition-site rule reaches them; see
[Why this is a use-site rule](#why-this-is-a-use-site-rule).

## Why this is a use-site rule

The alternative is to flag `impl Default for T` when the body is
effectful, which catches every consumer at once and is a smaller
trigger. It was considered and rejected here, for two reasons.

**It argues with published guidance.** The
[API Guidelines](https://rust-lang.github.io/api-guidelines/interoperability.html)
say it is "common and expected" for a type to implement both
`Default` and an empty `new`, "even if it is functionally identical
to `default`", and `clippy::new_without_default` is warn-by-default
and tells you to write `impl Default for Foo { fn default() -> Self
{ Foo::new() } }`. A `new` that does real work is universally
accepted, so default-on tooling actively manufactures the impls such
a rule would flag. This rule picks no such fight: it says only that
*this call* may surprise you.

**Its fix is a redesign; this one's is a line.** Flagging the impl
asks the author to restructure the type. Flagging the call offers
`mem::replace(&mut x, <explicit placeholder>)`, which is local and
mechanical. See [Suggested fix](#suggested-fix).

A definition-site rule is still worth having — it reaches the generic
and derive cases above — but as a separate rule with its own
rationale, not as this one wearing a different trigger.

## Examples

What decides each case is the constructed type's `Default`, so the
examples name their types and show the impl the verdicts turn on.
`SessionId` is the type from [Statement](#statement) with the rest of
its surface shown: its default mints a fresh identifier, which is a
difference in the value itself and so known non-ORT. `Job` is an
ordinary struct.

```rust
struct SessionId(Uuid);

impl SessionId {
    // The placeholder, for a session that has not started.
    const NIL: SessionId = SessionId(Uuid::nil());
    // The named constructor, for call sites that do want a new one.
    fn issue() -> Self { SessionId(Uuid::new_v4()) }
}

impl Default for SessionId {
    fn default() -> Self { SessionId::issue() }
}
```

**Avoid:** the placeholder mints an identifier nobody asked for.

```rust
fn rotate(session: &mut SessionId) -> SessionId {
    std::mem::take(session)
}
```

**Prefer:** name the placeholder, so the cost is where the reader is.

```rust
fn rotate(session: &mut SessionId) -> SessionId {
    std::mem::replace(session, SessionId::NIL)
}
```

**Not flagged:** `Vec`'s default is ORT, so the placeholder costs
nothing observable.

```rust
fn drain_queue(queue: &mut Vec<Job>) -> Vec<Job> {
    std::mem::take(queue)
}
```

**Not flagged:** `HashMap::default` mutates a thread-local seed on
every call, and is still ORT — the difference surfaces only through
an iteration order the contract declares arbitrary. See
[the worked contrast](./IMPLEMENTATION_CONVENTIONS.md#observational-referential-transparency).

```rust
fn drain_index(
    index: &mut HashMap<String, Job>,
) -> HashMap<String, Job> {
    std::mem::take(index)
}
```

**Not flagged by default:** the fallback tier, where the constructed
value is the one the caller wanted.

```rust
fn slot(
    ids: &mut HashMap<String, SessionId>,
    key: String,
) -> &mut SessionId {
    ids.entry(key).or_default()
}
```

**Not flagged:** written at the call site, so clause 1 fails. The
effect still happens — the derive runs `SessionId::default()` once,
measured — but it is reached through the derive rather than through
this call, which is
[a definition-site rule's job](#generic-code-and-derives-are-not-in-scope-either).

```rust
#[derive(Default)]
struct Config {
    verbose: bool,
    session: SessionId,
}

let config = Config { verbose: true, ..Default::default() };
```

## Suggested fix

Neither tier gets an autofix, and the diagnostic suggests a *shape*
rather than a value. The value is the author's to choose, and the
two forms a lint could mechanically reach for are both ones clippy
rewrites straight back.

For the placeholder tier, name the placeholder:
`mem::replace(&mut x, <explicit value>)` in place of
`mem::take(&mut x)`, and the `Cell` / `RefCell` equivalents. That
keeps the code's shape and puts the constructed value in the source
where a reader can see it.

For the fallback tier, name the constructor:
`unwrap_or_else(SessionId::issue)` in place of `unwrap_or_default()`.

### The named value must not be default-equivalent

Otherwise clippy undoes the fix. Measured against clippy 1.94, where
both lints are `style`, warn by default, and machine-applicable, so
`cargo clippy --fix` applies them without asking:

```rust
mem::replace(x, SessionId::default())  // --fix → mem::take(x)
o.unwrap_or_else(SessionId::default)   // --fix → unwrap_or_default()
```

Both results are what this rule flags. So a diagnostic phrased as
"write the construction out" would leave the user between two
warn-by-default lints, each undoing the other's fix. A named const
or constructor is untouched by either — `mem::replace(x,
SessionId::NIL)`, `mem::replace(x, SessionId::issue())` and
`o.unwrap_or_else(SessionId::issue)` all pass clippy clean — and
`Cell::replace` / `RefCell::replace` have no such lint at all.

The lint does not go looking for that constructor. Whether the type
spells it `new`, `create`, `issue` or `generate` is not something to
guess at, and the help text asks for one generically instead. A type
may have none, in which case the finding is about the type's design
rather than the call: either give it one, or record the decision
with
`#[expect(perfectionist::implicit_effectful_default, reason = "…")]`.

A third option belongs to the author rather than the call site: if
the effect was never meant to be reachable this way, the type wants a
trivial `Default` and a named constructor carrying the work. The
diagnostic may mention it; it must not suggest it, since it is a
change to a different file.

## Configuration

```toml
["perfectionist::implicit_effectful_default"]
# Placeholder constructors: the value they build is never read.
# On by default.
placeholder_constructors = true

# Fallback constructors: the value they build is the one the caller
# asked for, so the effect may well be intended. Off by default; see
# "Where the value is what you asked for".
fallback_constructors = false

# Paths whose `Default` is treated as observationally effectful,
# beyond the built-in denylist. Absolute, per the leading-`::`
# convention.
extra_effectful_types = []
ignore_effectful_types = []
```

The recognised constructors, which are not configurable — a project
that wants a different set wants a different rule:

| Constructor                        | Tier        | Runs when             |
|------------------------------------|-------------|-----------------------|
| `std::mem::take`                   | placeholder | always                |
| `Cell::take`                       | placeholder | always                |
| `RefCell::take`                    | placeholder | always                |
| `Option::unwrap_or_default`        | fallback    | the option is `None`  |
| `Result::unwrap_or_default`        | fallback    | the result is `Err`   |
| `Entry::or_default`                | fallback    | the entry is vacant   |
| `Option::get_or_insert_default`    | fallback    | the option is `None`  |

## Implementation notes

`LateLintPass::check_expr`, matching a call against the constructor
table by `DefId` and asking the shared classifier about the type it
constructs. Both halves are small; the classifier is the one that
needs care, and it belongs in the crate-internal module described
under
[Observational referential transparency](./IMPLEMENTATION_CONVENTIONS.md#observational-referential-transparency)
rather than in this rule, since a definition-site rule and
`default_assignment_after_take` want the same classification read
differently.

The type to classify is the constructed one, which for
`mem::take(&mut x)` is `x`'s type and for `entry(k).or_default()` is
the map's value type. Read it from typeck results rather than from
the written syntax.

### Difficulty

**Medium, and all of it is calibration.** The trigger is a `DefId`
match. The classifier is a denylist, so its quality is entirely a
question of which sources it recognises and how many false negatives
that leaves — which is a judgement to revisit against real findings,
not something to settle on paper.

### Evidence

None yet, and the file should not pretend otherwise. The hazard is
constructed rather than observed: no instance of an effectful
`Default` reached through a placeholder constructor has been found in
this repository or in the pnpm sources that prompted the
surrounding work. Worth holding until one turns up, or shipping and
seeing what a real codebase says.

## Default state

Active by default. The
[activation model](./IMPLEMENTATION_CONVENTIONS.md#rule-activation-model)
withholds a rule whose trigger is known to false-positive, and the
classification being a heuristic is not that — what matters is which
way it fails. *Known non-ORT* is a denylist, so a type it does not
recognise is simply not flagged: the unsoundness is silence, not
noise, and it costs findings rather than trust.

What remains is that the practice is legal, deliberate on the type
author's part, and sometimes exactly what the caller wanted. That is
a real cost, and it is what the tiers are for rather than a reason to
withhold the rule. `placeholder_constructors` is on, because a value
nobody reads is where an effect is unrequested outright.
`fallback_constructors` is a second opt-in, because there the
constructed value is the one the caller asked for.

The denylist does have a false-positive shape, and it is worth naming
so that calibration has something to aim at: a type that mirrors
`RandomState` — reaching a counter or a clock to seed something its
own contract declares arbitrary — is ORT and would be flagged. No
autofix and an `#[expect]` with a `reason` is what that case costs,
per [Suggested fix](#suggested-fix).

## Interaction with clippy and sibling rules

- **`clippy::new_without_default`** (`style`, warn by default) is
  upstream of the practice this rule flags: it asks a type with an
  effectful `new` to grow a `Default` delegating to it. Nothing to
  reconcile — this rule does not object to the impl — but a reader
  who wants to know why effectful `Default` impls are common should
  start there.
- **`clippy::unwrap_or_default`** (`style`, warn by default)
  rewrites `unwrap_or_else(Default::default)` and friends *to*
  `unwrap_or_default`, which moves code from visible to invisible
  construction. It reaches only the fallback tier, which is a second
  opt-in, so the disagreement is not the default experience.
- **`clippy::mem_replace_with_default`** (`style`, warn by default)
  rewrites `mem::replace(&mut x, T::default())` to `mem::take`,
  likewise trading a written construction for an implied one — and
  it lands in the placeholder tier, which is **on**. So this one is
  a live conflict rather than a deferred one: for a non-ORT `T`,
  clippy turns code this rule accepts into code it flags, and
  `cargo clippy --fix` does it silently. The two have a stable fixed
  point only because neither lint touches a replacement that is not
  default-equivalent, which is what
  [Suggested fix](#the-named-value-must-not-be-default-equivalent)
  requires the diagnostic to ask for.
- **`perfectionist::default_assignment_after_take`** reads the same
  classification in the opposite direction: it deletes a
  `Default::default()` call and so needs types that are *known ORT*,
  where this rule needs types that are *known non-ORT*. Neither
  answers the other; see
  [the two polarities](./IMPLEMENTATION_CONVENTIONS.md#the-two-polarities).

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
