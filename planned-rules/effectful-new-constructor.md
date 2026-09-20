# `effectful_new_constructor`

**Source:** project convention, matching the norm the Rust forums
converge on: a constructor that does something noteworthy should be
named for what it does, not `new`.

> [!IMPORTANT]
> This rule is `Inactive by default` because its fix is a rename; see
> [Default state](#default-state). Like its pair
> [`effectful_default_impl`](./effectful-default-impl.md), it rests
> on a heuristic that cannot be made sound, per
> [the two polarities](./IMPLEMENTATION_CONVENTIONS.md#the-two-polarities)
> — which bounds what its rustdoc may claim, and is not itself why it
> is withheld.

## Statement

A nullary `fn new() -> Self` reads as "give me the ordinary one". It
takes no arguments, so it cannot be answering a question; whatever it
produces is *the* value of that type, available on demand. And
`clippy::new_without_default` will ask it to become the type's
`Default`, which is the constructor the language reaches for
implicitly.

A nullary `new` that mints an identifier, reads a clock or connects
to something breaks both readings at once. The fix is a name that
says so.

```rust
// Avoid: reads as "the ordinary session", mints a fresh identity.
impl Session {
    pub fn new() -> Self { Session { id: Uuid::new_v4() } }
}

// Prefer: the name carries the promise.
impl Session {
    pub fn generate() -> Self { Session { id: Uuid::new_v4() } }
}
```

## Why restrict this?

This is a stylistic preference, not a correctness issue — and unlike
most rules in this catalogue, the preference is about a **name**
rather than about a construct. The effect is fine. Calling it `new`
is what this rule objects to.

Two things follow from that framing.

**Cost is not the complaint.** `Regex::new` compiles a regular
expression and nobody thinks it misnamed, because compiling is a pure
function of the input — the same pattern gives the same regex. Under
[the definition](./IMPLEMENTATION_CONVENTIONS.md#observational-referential-transparency)
allocation and cycles are invisible, so expense alone never triggers
this rule. The forum discussion agrees: *a single allocation doesn't
disqualify it. (`Box<T>` implements `Default`, after all…)*

**Arity is the scope.** The rule is confined to `fn new()` with no
parameters. A `new(args)` is answering a question and carries no
implicit-construction obligation — nothing calls it for you, and
`new_without_default` does not apply to it.

## What to lint

Flag an inherent `fn new() -> Self` when all of:

1. It takes **no parameters**. See *Arity is the scope* above.
2. Its body is **known non-ORT**, per
   [the denylist](./IMPLEMENTATION_CONVENTIONS.md#the-two-polarities).
   Unknown is not non-ORT; silence on unknown.
3. It is not itself a delegation to a differently-named constructor
   that would be reported instead.

Method name is `new` by default. `extra_constructor_names` widens it
for a project with a house convention, but the default list is one
entry on purpose: `with_capacity`, `from_*` and the rest carry their
own promises, and a rule that assumed otherwise would be guessing.

### Exemptions

- Trait methods, whose names the implementer did not choose.
- Test code and build scripts by default, per
  [Recognising test-exclusive code](./IMPLEMENTATION_CONVENTIONS.md#recognising-test-exclusive-code).
- Proc-macro-synthesised items, per
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).

## Examples

**Avoid:** nullary, and the identity is fresh each call.

```rust
impl Session {
    pub fn new() -> Self { Session { id: Uuid::new_v4() } }
}
```

**Prefer:** renamed, and `Default` — if the type wants one — becomes
uneventful.

```rust
impl Session {
    pub fn generate() -> Self { Session { id: Uuid::new_v4() } }
}
```

**Not flagged:** takes an argument, so clause 1 fails. Expensive and
ORT besides.

```rust
impl Regex {
    pub fn new(pattern: &str) -> Result<Self, Error> { /* compiles */ }
}
```

**Not flagged:** nullary and expensive, but ORT — allocation is
invisible under the definition.

```rust
impl Buffer {
    pub fn new() -> Self { Buffer(Vec::with_capacity(4096)) }
}
```

## Suggested fix

Rename, to a verb that carries the promise the body makes:
`generate` for a fresh identity, `open` or `connect` for something
external, `now` for a clock read. `File::open` rather than
`File::new` is the shape std itself uses, and renaming is what the
forum thread behind this rule settled on.

The lint cannot choose the verb, so this is a note rather than a
structured suggestion. A rename is also a breaking change for a
published crate, which the diagnostic should say.

## Configuration

```toml
["perfectionist::effectful_new_constructor"]
# Constructor names to inspect, beyond `new`. Empty by default; see
# "What to lint".
extra_constructor_names = []

# Whether test code / a build script is exempt. Both default to
# `true`.
exempt_tests = true
exempt_build_scripts = true

# Paths treated as observationally effectful beyond the built-in
# denylist, and paths to remove from it. Absolute, per the
# leading-`::` convention.
extra_effectful_paths = []
ignore_effectful_paths = []
```

## Implementation notes

`LateLintPass::check_impl_item` over inherent `fn`s, filtered by name
and arity, then the shared classifier from
[Observational referential transparency](./IMPLEMENTATION_CONVENTIONS.md#observational-referential-transparency).
Everything expensive is in the classifier, which is shared with
[`effectful_default_impl`](./effectful-default-impl.md) and read the
same direction — both want *known non-ORT*.

### Difficulty

**Medium**, and for the same reason as its pair: the trigger is a
name-and-arity match, and the classifier is the whole of it.

### Evidence

None. Unlike the rest of this file's reasoning, which follows a
documented community norm, the rule has no observed finding behind
it — no nullary effectful `new` has been identified in this
repository or in the sources that prompted the surrounding work.

## Default state

Inactive by default. The practice is legal and widespread, and the
fix is a rename — a breaking change for anything published. A rule
whose remedy costs a major version is one a project should choose
deliberately.

The classification being a heuristic is not among the reasons, for
the reason its pair
[sets out](./effectful-default-impl.md#default-state): a denylist
under-fires, so it costs findings rather than trust.

## Interaction with clippy and sibling rules

- **`clippy::new_without_default`** (`style`, warn by default) is the
  bridge between this rule and its pair. Given an effectful nullary
  `new`, that lint asks for a `Default` delegating to it — turning a
  finding of this rule into a finding of
  [`effectful_default_impl`](./effectful-default-impl.md). Acting on
  this rule first removes the `new()` that lint keys on, so the two
  perfectionist rules are best adopted in that order.
- **`clippy::should_implement_trait`** (`style`) flags an inherent
  method shadowing a std trait method's name. Different concern,
  overlapping surface.
- **`perfectionist::effectful_default_impl`** is this rule's pair;
  the precedence between them is settled in
  [that file](./effectful-default-impl.md#precedence-between-the-two).
- **`perfectionist::implicit_effectful_default`** flags the call
  sites that reach an effectful `Default` implicitly, rather than
  either definition site.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.
