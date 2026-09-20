# `effectful_default_impl`

**Source:** project convention. A hand-written `impl Default` whose
`default()` is observationally effectful, flagged where it is
written rather than where it is reached.

> [!IMPORTANT]
> This rule contradicts advice the ecosystem ships on by default; see
> [What this rule is arguing with](#what-this-rule-is-arguing-with).
> It also rests on a heuristic that cannot be made sound, per
> [the two polarities](./IMPLEMENTATION_CONVENTIONS.md#the-two-polarities).
> Both are reasons it is `Inactive by default`, and reasons to weigh
> it against real findings before enabling it anywhere.

## Statement

`Default::default()` is the one constructor Rust calls on a
programmer's behalf. `std::mem::take`, `Cell::take`,
`unwrap_or_default`, `or_default`, `#[derive(Default)]` on an
enclosing struct, and any generic bounded by `T: Default` all invoke
it without the calling code naming it.

A type that accepts that trait is therefore promising that being
constructed is uneventful. An `impl Default` that mints an
identifier, reads a clock or advances a counter breaks the promise
for every one of those callers at once, and none of them can see it.

```rust
// Every `mem::take` of a `Session` anywhere now mints an id.
impl Default for Session {
    fn default() -> Self { Session { id: Uuid::new_v4() } }
}
```

The remedy is to make `default()` trivial and move the work to a
named constructor, so that code wanting a fresh identifier asks for
one by name.

```rust
impl Session {
    fn generate() -> Self { Session { id: Uuid::new_v4() } }
}
```

## Why restrict this?

This is a stylistic preference, not a correctness issue. The impl is
legal, the author chose it, and callers may want exactly what it
does.

The objection is that `Default` is reached implicitly, so the cost of
an effect in it is paid at call sites that never mention it and
cannot opt out. A named constructor carries the same work with the
reader's consent. That argument is about *where the effect is
visible*, not about the effect itself — which is why the fix is a
rename and a delegation rather than removing anything.

## What to lint

Flag an `impl Default for T` when both:

1. The impl is **hand-written**. A `#[derive(Default)]` is not
   flagged even where a field's `Default` is effectful: the derive
   faithfully composes what its fields do, and the finding belongs to
   whichever field type introduced the effect.
2. The body is **known non-ORT**, per
   [the denylist](./IMPLEMENTATION_CONVENTIONS.md#the-two-polarities).
   Unknown is not non-ORT; silence on unknown.

An impl that merely delegates — `fn default() -> Self { Self::new() }`
— is classified by what `new` does, since that is where the effect
lives. Where that fires, the finding is usually
[`effectful_new_constructor`](./effectful-new-constructor.md)'s
rather than this rule's, and the two should not both report the same
type; see
[Precedence](#precedence-between-the-two).

### Exemptions

- Test code and build scripts by default, per
  [Recognising test-exclusive code](./IMPLEMENTATION_CONVENTIONS.md#recognising-test-exclusive-code).
  A fixture whose `Default` counts constructions is a testing device,
  not a design.
- Proc-macro-synthesised impls, per
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).
  A derive from another crate is that crate's business.

## Examples

**Avoid:** the identifier is minted wherever the type is defaulted.

```rust
impl Default for Session {
    fn default() -> Self {
        Session { id: Uuid::new_v4(), started: Instant::now() }
    }
}
```

**Prefer:** the work is named, and `Default` stays uneventful.

```rust
impl Session {
    fn generate() -> Self {
        Session { id: Uuid::new_v4(), started: Instant::now() }
    }
}

impl Default for Session {
    fn default() -> Self {
        Session { id: Uuid::nil(), started: Instant::now() }
    }
}
```

Where no uneventful value exists, the type should not implement
`Default` at all — which is the conclusion the forum threads behind
[What this rule is arguing with](#what-this-rule-is-arguing-with)
reach as well.

**Not flagged:** `HashMap`'s default mutates a thread-local seed on
every call and is still ORT, because the difference surfaces only
through an iteration order the documentation calls arbitrary. A rule
keyed on "the body touches a static" would flag std here; this one
does not. See
[the worked contrast](./IMPLEMENTATION_CONVENTIONS.md#observational-referential-transparency).

**Not flagged:** allocation is not an effect under the definition,
so an expensive-but-ORT default is fine.

```rust
impl Default for Buffer {
    fn default() -> Self { Buffer(Vec::with_capacity(4096)) }
}
```

**Not flagged:** derived, so clause 1 fails. If `Session`'s own
`Default` is effectful the finding is reported there.

```rust
#[derive(Default)]
struct Run { name: String, session: Session }
```

## What this rule is arguing with

Worth stating in full, because a reader who enables this rule will
meet the other side of it immediately.

The [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/interoperability.html)
say it is "common and expected for types to implement both `Default`
and an empty `new` constructor… even if it is functionally identical
to `default`". `std`'s own documentation for `Default` says only that
it gives a type "a useful default value", and imposes no condition on
cost or effects. And `clippy::new_without_default` is warn-by-default
and its documented remedy is:

```rust
impl Default for Foo {
    fn default() -> Self { Foo::new() }
}
```

Since a `new` doing real work is universally accepted, that lint
manufactures the impls this rule flags. The two are in direct
tension, and a project enabling this rule should expect to disagree
with a default-on Clippy lint on the same types.

The counterweight is that the community norm, where it is stated at
all, matches this rule: the discussions land on `Default` being
trivial and deterministic, with renaming the constructor as the
accepted fix. That norm is folklore rather than published guidance,
which is precisely why this rule ships off.

## Precedence between the two

`effectful_default_impl` and
[`effectful_new_constructor`](./effectful-new-constructor.md) will
often see the same type, because the delegating impl Clippy asks for
puts one effect behind two doors. Reported twice it is one design
decision with two names.

So a delegating `Default` — a body that does nothing but call a
nullary constructor — is left to the other rule, which is where the
effect is written. A `Default` with its own effectful body is this
rule's, whether or not a `new` also exists.

## Configuration

```toml
["perfectionist::effectful_default_impl"]
# Whether test code / a build script is exempt. Both default to
# `true`; see "Exemptions".
exempt_tests = true
exempt_build_scripts = true

# Paths treated as observationally effectful beyond the built-in
# denylist, and paths to remove from it. Absolute, per the
# leading-`::` convention.
extra_effectful_paths = []
ignore_effectful_paths = []
```

## Implementation notes

`LateLintPass::check_item`, matching `ItemKind::Impl` whose trait ref
is `Default`, then asking the shared classifier about the body. The
classifier is the crate-internal module described under
[Observational referential transparency](./IMPLEMENTATION_CONVENTIONS.md#observational-referential-transparency);
this rule reads its *known non-ORT* answer, and
`perfectionist::default_assignment_after_take` reads the *known ORT*
one, so neither may be implemented as the other's negation.

Recognising a hand-written impl from a derived one is a span check:
a derive-generated impl comes from a `MacroKind::Derive` expansion.

### Difficulty

**Medium, and all of it is the classifier.** The trigger is an item
match. Whether the rule is any good is entirely a question of which
effect sources the denylist recognises, which is calibration against
real findings rather than something to settle on paper.

### Evidence

None. No effectful `Default` impl has been found in this repository
or in the pnpm sources that prompted the surrounding work; the
`Session` and `Ticket` shapes in this file are constructed. The rule
is a considered proposal, not a response to an observed problem, and
should be weighed accordingly.

## Default state

Inactive by default, for three reasons rather than the usual one: the
classification is a heuristic, the practice is legal and sometimes
intended, and the rule disagrees with a warn-by-default Clippy lint
on the same types. The
[activation model](./IMPLEMENTATION_CONVENTIONS.md#rule-activation-model)
reserves `Inactive` for rules whose preferred configuration varies by
project, and a rule that contradicts published guidance is squarely
that.

## Interaction with clippy and sibling rules

- **`clippy::new_without_default`** (`style`, warn by default) is in
  direct tension with this rule; see
  [What this rule is arguing with](#what-this-rule-is-arguing-with).
- **`clippy::derivable_impls`** (`complexity`) flags a hand-written
  `Default` that could be derived. Its findings can never be this
  rule's, since a derivable body is ORT by construction.
- **`perfectionist::implicit_effectful_default`** flags the *call
  sites* that reach an effectful `Default` without naming it. Same
  hazard, opposite end: that rule asks the caller to name the
  placeholder, this one asks the author to name the constructor.
  Enabling both on one codebase reports the same effect twice, from
  two directions, which may be what a project wants or may be noise —
  a reason to adopt one first.
- **`perfectionist::effectful_new_constructor`** is this rule's pair;
  see [Precedence](#precedence-between-the-two).

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
