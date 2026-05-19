# `serde_wrapper_style`

**Default state:** `active`  
**Source:** project convention.

## Statement

When a wrapper type delegates its serde representation to an inner
type, two attribute forms produce the same wire output:

```rust
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct A(Inner);

// vs.

#[derive(Serialize, Deserialize)]
#[serde(from = "Inner", into = "Inner")]
struct B(Inner);
```

The two are **not** semantically equivalent in general:

- `transparent` is zero-cost. The serde codegen erases the wrapper
  and goes straight to the inner type's `Serialize` / `Deserialize`.
- `from`/`into` round-trips through a fresh `Inner` value at every
  (de)serialization, calling `From<Inner> for Self` and
  `From<Self> for Inner`. The conversion is the *whole point* — it
  is where validation, normalisation, or splitting lives.
- `transparent` inherits the inner type's borrowing behaviour
  (`Cow<'de, str>` borrows from input where possible);
  `from = "T"` ties the borrow story to `T`.

The two are *operationally* interchangeable only when the
`From<Inner>` and `From<Self> for Inner` impls are trivial moves
(no validation, no normalisation, just constructing or unwrapping
the wrapper). In that narrow case, a project can pick one form and
enforce it consistently.

This rule enforces that project preference when (and only when) the
two forms would produce identical behaviour.

## Configuration

```toml
[serde_wrapper_style]
style = "preserve"
# "preserve"          — no-op (default).
# "transparent"       — flag trivial `#[serde(from, into)]` and
#                       suggest `#[serde(transparent)]`. Existing
#                       non-trivial `from`/`into` (where the
#                       conversion does real work) is left alone.
# "from_into"         — flag `#[serde(transparent)]` and suggest
#                       `#[serde(from = "T", into = "T")]`,
#                       synthesising trivial `From` / `Into` impls
#                       in the suggestion. Suitable for projects
#                       that want every wrapper ready to grow a
#                       validation hook later.
```

## Style: `transparent`

```rust
// Bad (under style = "transparent")
#[derive(Serialize, Deserialize)]
#[serde(from = "Inner", into = "Inner")]
struct A(Inner);

impl From<Inner> for A {
    fn from(value: Inner) -> Self { Self(value) }
}
impl From<A> for Inner {
    fn from(wrapper: A) -> Self { wrapper.0 }
}

// Good
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct A(Inner);
```

The lint *does not* fire when the `From` impls do anything beyond
trivially construct/destruct:

```rust
// Not flagged: the From impl validates.
#[serde(from = "Inner", into = "Inner")]
struct ValidatedPort(u16);

impl From<Inner> for ValidatedPort {
    fn from(value: Inner) -> Self {
        assert!(value.0 != 0);
        Self(value.0)
    }
}
```

## Style: `from_into`

```rust
// Bad (under style = "from_into")
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct A(Inner);

// Good
#[derive(Serialize, Deserialize)]
#[serde(from = "Inner", into = "Inner")]
struct A(Inner);

impl From<Inner> for A {
    fn from(value: Inner) -> Self { Self(value) }
}
impl From<A> for Inner {
    fn from(wrapper: A) -> Self { wrapper.0 }
}
```

The autofix synthesises the trivial impls; the project owner has
opted into this form precisely so the impls exist and can grow into
real validation later.

## What to lint

For each `struct` or `enum-variant` carrying a serde derive:

1. Read its serde attributes (`#[serde(...)]`) to determine its
   current form: `transparent`, `from`/`into`, or neither.
2. If the type is not single-field (one tuple-struct field, one
   named field, or — for the variant case — one variant-payload
   field), bail. `transparent` has no equivalent for multi-field
   shapes.
3. Branch on `style`:
   - **`transparent`**: only fires on `from`/`into` form. Locate
     the `From<Inner> for Self` and `From<Self> for Inner` impls in
     the same crate; if both are *trivial* by the recogniser below,
     suggest `#[serde(transparent)]` and removal of the two impls.
   - **`from_into`**: only fires on `transparent` form. Suggest
     swapping to `#[serde(from = "Inner", into = "Inner")]` and
     synthesise the two trivial impls.

### Trivial-impl recogniser

A `From<X> for Y` impl is *trivial* if the `from(value)` body is
exactly one of:

- `Y(value)` (tuple-struct constructor with `Y == Self`).
- `Self(value)`.
- `Y { field: value }` (struct-expression with one field).
- `Self { field: value }`.
- `EnumName::Variant(value)` (enum variant constructor) when the
  containing type is the enum and the variant has one payload.

A `From<Y> for X` impl is trivial if `from(wrapper)` is exactly
`wrapper.0` (tuple), `wrapper.field` (named), or
`match wrapper { Variant(x) => x }` for the enum case.

Anything else — including a body that uses `?`, calls `into()`,
performs a cast, validates, or has any preceding `let` binding — is
non-trivial. Bail.

## Implementation notes

- `LateLintPass`. The lint is local-only: it can only act when the
  type *and* its `From`/`Into` impls are in the current crate.
  An impl re-exported from another crate is invisible to a Dylint
  pass and the lint must stay silent.
- Detection of the current attribute form:
  - **`transparent`**: any `#[serde(transparent)]` on the item.
  - **`from`/`into`**: a single `#[serde(from = "T", into = "T")]`
    where the two `T` strings parse to the same Rust type and that
    type is structurally the wrapper's inner field type. The
    "structurally" check is the same path-and-generic comparison
    used by [`serde-source-types`](./serde-source-types.md); share
    the helper.
  - **Mixed/non-matching**: bail. A `from = "A", into = "B"` with
    different types is doing something deliberately asymmetric and
    is not interchangeable with `transparent`.
- Detection of the trivial-impl shape uses
  `clippy_utils::higher`-style HIR pattern matchers. The recogniser
  must match exactly the bodies listed above; any other body shape
  flips the impl to "non-trivial" and disables the suggestion.
- **Parser style.** The serde-attribute argument parser
  (`from = "T"`, `into = "T"`, `transparent`) is non-trivial.
  Implement it as parser-combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md).
  Reuse the type-literal parser from
  [`serde-source-types`](./serde-source-types.md) for the
  `"Inner"` strings.

### Difficulty and risk

**Difficulty: hard.** Comparable to
[`prefer-derive-more`](./prefer-derive-more.md)'s `display`
sub-lint:

- *Body-shape detection* — small, well-defined patterns. The
  trivial-impl recogniser is a finite list and can be tested
  exhaustively.
- *Cross-impl coordination* — the lint must locate two impls that
  belong together (`From<Inner> for Self` and
  `From<Self> for Inner`) and confirm both are trivial. Either
  impl missing means `from`/`into` shouldn't even compile, so the
  lint can rely on rustc to have rejected that case earlier.
- *Type-equality check* — the inner field's declared type must
  equal the `T` written inside `from = "T"`. String-equality is
  too strict (whitespace, paths) and HIR-equality requires the
  attribute string to round-trip through the type checker, which
  serde does for its own purposes. Reuse the comparison logic
  needed by `serde-source-types`.

**Risk: high.** A false positive in style `transparent` silently
disables a `From` impl that was actually doing work — exactly the
behavioural change `transparent` was avoided to prevent. Default
off; treat as experimental until exercised.

False negatives (the lint misses an interchangeable pair) are
benign: the project keeps the form it already had.

### What the synthesised `from_into` form looks like

When `style = "from_into"` fires on a `#[serde(transparent)]`
wrapper, the suggestion writes:

```rust
#[derive(Serialize, Deserialize)]
#[serde(from = "Inner", into = "Inner")]
struct A(Inner);

impl From<Inner> for A {
    fn from(value: Inner) -> Self { Self(value) }
}
impl From<A> for Inner {
    fn from(wrapper: A) -> Self { wrapper.0 }
}
```

The two impl blocks are appended at the end of the same module.
Applicability is `MaybeIncorrect` because:

1. The project may already have a manual `From` impl elsewhere; the
   lint can't easily detect duplicates across modules.
2. The `Inner` type's path may need adjustment in the new module
   context.

The autofix is offered as help text; `cargo clippy --fix` will not
apply it without manual review.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Default state

The default `style = "preserve"` keeps the
pass a no-op so the rule is zero-friction to adopt. A project
that has audited the trivial-impl recogniser on its codebase opts
in by setting `style` to `transparent` or `from_into`.

## Why a single rule instead of two

The two directions are duals on the same axis: "should a
single-field serde wrapper be `transparent` or `from`/`into`?" A
project answers that question once and the lint enforces it. Two
separate rules (`prefer-serde-transparent` and
`prefer-serde-from-into`) would have to coordinate to never both
fire on the same item, since each direction's "good" form is the
other direction's "bad" form. Combining them into one rule with a
`style` knob keeps the policy expressible in one place.

## Interaction with [`serde-source-types`](./serde-source-types.md)

`serde-source-types` decides the *type* of `T` in
`#[serde(from = "T", into = "T")]` (forbidding `&'de str`,
suggesting `Cow<'de, str>` vs. `String`).

`serde-wrapper-style` decides whether the `from`/`into` form is the
right shape at all, or whether `transparent` would do.

The two rules layer cleanly and may both fire on the same attribute
when the project has chosen `transparent` style but the existing
`from`/`into` form *also* uses a problematic source type. The
diagnostics complement each other.
