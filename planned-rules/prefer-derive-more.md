# `prefer_derive_more`

**Source:** project convention. AI assistants frequently expand
`#[derive(From)]` / `#[derive(Display)]` patterns into hand-rolled
`impl` blocks, particularly in code that began life inside a chat
window where the author asked for "a `From` impl for `Foo`" instead
of "add `From` to the derive list".

## Statement

When an `impl` block reproduces a pattern that
[`derive_more`](https://docs.rs/derive_more) can express as a derive,
flag the `impl` and suggest the derive instead.

The rule is split into one sub-lint per `derive_more` trait so a
project can enable the trivially-mechanical patterns while leaving
the hard ones (`Display`, `Error`) opt-in until tested in real
codebases.

## Sub-lints

### Easy: pattern matches a fixed AST shape

`From`, `Into`, `AsRef`, `AsMut`, `Deref`, `DerefMut`. Each one is
recognised by a small set of body shapes:

- **`from`**: `impl From<A> for B { fn from(v: A) -> Self { S(v) } }`
  where `S` is `B`, `Self`, a tuple-struct constructor, an enum
  variant constructor, or a `B { field: v }` struct expression.
  Replace with `#[derive(From)]` on `B` and (for multi-field
  variants) `#[from]` on the relevant field.
- **`as_ref`** / **`as_mut`**:
  `fn as_ref(&self) -> &T { &self.field }` (or `&self.0`).
  Replace with `#[derive(AsRef)]` plus a `#[as_ref(forward)]` or
  field-level `#[as_ref]` if disambiguation is needed.
- **`deref`** / **`deref_mut`**:
  ```rust
  type Target = T;
  fn deref(&self) -> &T { &self.field }
  ```
  Replace with `#[derive(Deref)]`.
- **`into`**: `impl Into<B> for A` (note: the standard library's
  blanket `From → Into` makes a hand-written `Into` impl unusual).
  derive_more's `Into` is placed on the *source* type. The lint
  primarily suggests `#[derive(Into)]` on `A`, but for a body of
  shape `B::wrap(self)` it also flags the inverted form
  `impl From<A> for B`.

Detection difficulty: **easy**. Each pattern matches a body of two
or three nodes; `clippy_utils::path_to_local`-style helpers cover
the field-access and constructor recognition. Autofix is
`MachineApplicable` for the unambiguous newtype cases and
`MaybeIncorrect` when the impl carries unusual where-clauses or the
struct has multiple fields whose roles are not obvious.

### Medium: pattern is uncommon or limited

`TryFrom`, `TryInto`. derive_more's `#[derive(TryFrom)]` covers only:

- Tuple structs with a single inner type, where the conversion is
  the inverse of an existing `From`-style wrapping.
- `#[repr(Int)]` enums with explicit discriminants, mapping the
  integer back to the variant.

Hand-written `TryFrom` impls outside those shapes cannot be derived.
The lint should detect the in-shape cases conservatively and emit
help-only when the impl body matches one of the recognised
patterns; otherwise stay silent.

Detection difficulty: **medium**. The body shape is small but the
applicability check (does the struct/enum match the supported
shape?) is the bulk of the work.

### Hard: `Display`

`impl fmt::Display for B { fn fmt(&self, f: &mut fmt::Formatter)
-> fmt::Result { write!(f, "literal", args...) } }`.

Replace with `#[derive(Display)]` and `#[display("literal", args)]`
on the struct/variant.

Detection requires:

1. The body is exactly one expression statement (no preceding
   `let`s, no early returns, no match arms).
2. The expression is `write!(f, ...)` or `f.write_str("...")` where
   `f` is the formatter parameter — not a free variable, not a
   shadowed binding.
3. The format string is a single string literal, not a `concat!()`
   or runtime expression.
4. Each argument translates to derive_more's expression syntax:
   - Positional fields: `self.0`, `self.1` → `_0`, `_1`.
   - Named fields: `self.field` → `field` (only when the name does
     not collide with a method call site reading like a path).
   - Method calls on `self`: kept verbatim if derive_more accepts
     them (it does for stable forms).
   - Calls on free locals or external functions: not translatable;
     the lint must not fire.
5. No format-spec arguments use `..self` capture or other
   non-derivable shapes.

Detection difficulty: **hard.** This is the sub-lint most likely to
false-positive, both because the body shapes that look reducible
sometimes hide subtle differences (e.g., an early-bail `if` that
returns a different message) and because derive_more's expression
DSL is a moving target across major versions. Default off.

### Hardest: `Error`

`impl std::error::Error for B { fn source(&self) -> Option<&(dyn
Error + 'static)> { Some(&self.field) } }`. Replace with
`#[derive(Error)]` and a `#[source]` annotation on the field.

Detection requires:

1. The set of overridden methods (`source`, `description` (deprecated),
   `cause` (deprecated), `provide`) maps onto derive_more's
   attribute vocabulary (`#[source]`, `#[from]`, `#[error(forward)]`,
   `#[error(ignore)]`, `#[error(not(source))]`).
2. The body of each overridden method is in the small recognised
   set: `Some(&self.field)`, `self.field.source()`, `None`, etc.
3. The struct or enum's field topology lines up — e.g., a `#[from]`
   field must match a `From` impl that already exists.
4. Cross-trait coordination: a hand-rolled `Error` impl is often
   paired with a hand-rolled `Display` impl. The two suggestions
   must compose (or both be skipped if either is non-trivial).

Detection difficulty: **hardest.** The Error trait's interaction
with `Display`, `From`, and the `provide` API makes this the most
brittle of the sub-lints. Default off; treat as experimental until
exercised on real codebases.

## Configuration

```toml
[prefer_derive_more]
# Easy sub-lints — on by default.
from       = true
into       = true
as_ref     = true
as_mut     = true
deref      = true
deref_mut  = true

# Medium — on by default; conservatively only fires on supported
# shapes.
try_from   = true
try_into   = true

# Hard — off by default. Enable per project once the lint has been
# vetted against a real codebase.
display    = false
error      = false

# Path to the derive_more crate as imported by the project. Most
# projects use the unqualified name; some re-export under a
# workspace name.
crate_path = "derive_more"
```

## Implementation notes

- `LateLintPass::check_item` on `ItemKind::Impl`. Walk the impl
  header to identify the trait being implemented; match against the
  configured sub-lint set.
- For every sub-lint, the body-shape check is a shallow HIR pattern
  match. Reuse `clippy_utils::higher::*` helpers where they apply.
- Suggested edits write a new attribute on the type definition.
  This requires the type's `DefId` to live in the same crate (cannot
  modify a re-exported foreign type), so the lint is local-only.
- For each sub-lint, the fix:
  - **Removes** the entire `impl` block.
  - **Adds** the derive to the type's existing `#[derive(...)]`
    attribute (or creates one if absent).
  - **Adds** any field-level annotation (`#[source]`, `#[from]`,
    `#[as_ref]`, `#[display(...)]`) needed.
- The diagnostic emits two spans: the impl block (to be removed)
  and the type definition (where the derive lands). Both spans are
  needed for the autofix to apply atomically.
- **Caveats by sub-lint**:
  - `display`: the `write!(f, "{}", x)` → `#[display("{}", x)]`
    rewrite must preserve every format-spec exactly (`{:?}`, `{:>5}`,
    `{:#x}`). Bail out on any format spec that derive_more cannot
    represent.
  - `error`: the `source` body recognition is the high-false-positive
    risk. Conservative starting set: `Some(&self.field as &dyn
    Error)`, `Some(&self.field)` with field type implementing
    `Error`. Bail on anything else.
  - `from`: an impl that performs *non-trivial* work in the body
    (validating `v` before constructing `B(v)`) is not derivable.
    Bail on any body that does not consist of a single constructor
    expression.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Severity

Warn for the easy and medium sub-lints. The hard sub-lints, when
enabled, default to warn but should be promoted to deny only after
the project has audited their suggestions on a representative
sample.

## Why this rule exists

Hand-written `impl From for ...` blocks are the most common
AI-generated alternative to a one-line derive. They sneak past code
review because they look "more explicit" — the model produced four
lines of code where one attribute would do. The lint catches them
mechanically and pushes the codebase toward the smaller, derive-
based form.

The `Display` and `Error` sub-lints sit behind opt-in flags because
the cost of a false positive is high: rewriting a hand-rolled error
type incorrectly produces a compile error or, worse, a runtime
behaviour change. Enable them once and audit; disable again if the
warning rate is too noisy.
