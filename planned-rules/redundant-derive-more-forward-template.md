# `redundant_derive_more_forward_template`

**Source:** project convention. Distilled from a review thread on
[`HoangVanKhai/my-translated-lyrics#133`](https://github.com/HoangVanKhai/my-translated-lyrics/pull/133),
where a `#[display("{_0}")]` on a single-field error newtype was
removed as a restatement of what the derive does anyway. Sibling to
[`uninlined-derive-more-args`](./uninlined-derive-more-args.md) and
[`overly-long-derive-more-template`](./overly-long-derive-more-template.md),
which reshape a template that does real work; this one deletes a
template that does none.

## Statement

A `derive_more` formatting derive already forwards to the inner
value when the container holds exactly one field. A `#[display(...)]`
attribute whose template does nothing but that forward is a longhand
spelling of the derive's own default, and expands to the identical
call.

**Avoid:**

```rust
use derive_more::Display;

#[derive(Display)]
#[display("{_0}")]
struct SanitizedHtml(String);
```

**Prefer:**

```rust
use derive_more::Display;

#[derive(Display)]
struct SanitizedHtml(String);
```

The same holds for a named single field (`#[display("{only}")]`), for
the un-inlined argument forms (`#[display("{}", _0)]`,
`#[display("{}", self.0)]`, `#[display("{x}", x = _0)]`), and for a
single-field enum variant.

## Why restrict this?

This is a stylistic preference, not a correctness issue: the
attribute compiles to exactly the code the derive emits without it.
It is the same shape of dead weight as an unused import or a needless
borrow — it restates what the compiler already does, and a reader has
to read the whole template before concluding it changes nothing.

The one consequence past tidiness is worth knowing. Because the
attribute names the field, a container that later grows a second
field keeps compiling and silently renders only the first; the bare
derive fails to compile there instead.

## What to lint

`derive_more` compiles a formatting template that is *nothing but a
single unadorned placeholder* into a direct call to the placeholder's
formatting trait, rather than into a `write!`. That is the same shape
the derive emits for a single-field container with no attribute at
all, which is what makes the attribute removable — and what makes
"looks like a forward" too loose a test. The trigger is the narrower
"compiles to the same call".

Flag an attribute when **all** of these hold:

1. **It is a `derive_more` formatting attribute.** The container
   carries a `#[derive(...)]` naming one of the `Display`-like
   derives, and the attribute's name is that derive's attribute name:

   | Derive     | Attribute           | Placeholder type |
   |------------|---------------------|------------------|
   | `Binary`   | `#[binary(...)]`    | `{:b}`           |
   | `Display`  | `#[display(...)]`   | `{}`             |
   | `LowerExp` | `#[lower_exp(...)]` | `{:e}`           |
   | `LowerHex` | `#[lower_hex(...)]` | `{:x}`           |
   | `Octal`    | `#[octal(...)]`     | `{:o}`           |
   | `Pointer`  | `#[pointer(...)]`   | `{:p}`           |
   | `UpperExp` | `#[upper_exp(...)]` | `{:E}`           |
   | `UpperHex` | `#[upper_hex(...)]` | `{:X}`           |

2. **The template is one placeholder and nothing else.** No literal
   text before, after, or between; no second placeholder; no escaped
   brace.

3. **The placeholder is unadorned.** No fill, alignment, sign, `#`,
   zero-padding, width, or precision. Its type is either absent or
   the trivial trait selector from the table above; `{:x?}` and
   `{:X?}` are excluded (they select a modified `Debug`, not a
   forward).

4. **The placeholder's trait is the derived trait.** `{}` selects
   `Display`, `{:x}` selects `LowerHex`, and so on — from the
   placeholder's own type, *not* from the attribute's name. This is
   the check that keeps `#[lower_hex("{_0}")]` unflagged: the bare
   `{}`-shaped placeholder forwards to `Display`, so the attribute
   really does change what the derive would have done.

5. **The placeholder's argument is the value the derive would have
   forwarded anyway** — see the two triggers below.

6. **Removing the attribute leaves that forward in place**, rather
   than exposing a different template — see the enum caveat below.

### Trigger: a single-field container restating its field

The attribute sits on a struct, or on an enum variant, with exactly
one field, and the placeholder's argument resolves to that field:
`_0` for a tuple field, the field's name (unraw-ed, so `{type}`
matches `r#type`) for a named one, or `self.0` / `self.field` written
out as an argument.

The borrowed and dereferenced spellings of those arguments —
`#[display("{}", *_0)]`, `#[display("{}", &self.0)]` — forward
identically, because `derive_more` wraps an argument it cannot match
to a field in `&(...)` and the blanket `impl Display for &T` collapses
the extra reference. Recognising them is optional: they are rare, and
leaving them out costs only a missed diagnostic.

### Trigger: an enum restating `{_variant}`

An enum-level `#[display("{_variant}")]` is the container-level
counterpart: `{_variant}` under the derived trait is exactly what
each variant is formatted with when the enum carries no shared
template, so `derive_more` treats the attribute as absent. Removing
it cannot change any variant's output.

### Not flagged

- **A container with zero or more than one field.** With more than
  one, the template is mandatory; with zero (a unit struct or unit
  variant) there is no field to forward to.
- **`#[debug(...)]`.** `derive_more`'s `Debug` derive defaults to the
  struct-shaped `Wrapper("inner")` builder output, not to a forward,
  so `#[debug("{_0:?}")]` genuinely changes the rendering and is
  never redundant. It is excluded from the table above for this
  reason.
- **A placeholder selecting a different trait than the derive.**
  `#[display("{_0:?}")]` forwards to `Debug`; `#[lower_hex("{_0}")]`
  forwards to `Display`. Both differ from the default forward.
- **Any adorned placeholder.** `#[display("{_0:>8}")]` is not a
  forward at all — it applies its own width instead of passing the
  caller's format spec through.
- **A variant under a non-wrapping enum-level template.** When an
  enum carries `#[display("...")]` whose template does *not* mention
  `{_variant}`, that template is what a variant falls back to. A
  variant's own `#[display("{_0}")]` overrides it, so deleting the
  variant attribute changes the output to the enum-level text.
  Removing a variant attribute is only safe when the enum has no
  enum-level template, or its template mentions `{_variant}`.
- **A field whose presence is `cfg`-dependent.** A `#[cfg(...)]` on a
  field, or a `#[cfg_attr(..., display(...))]` whose predicate does
  not always hold, can make the field count differ between
  configurations. Bail rather than guess.

### A generic container is not a bail-out

The one shape that looks like it should bail and does not.
`derive_more` infers a formatting bound for every type parameter a
template interpolates directly, so deleting a template raises the
question of whether the bound goes with it:

```rust
#[derive(Display)]
enum StringOrNumber<Number> {
    #[display("{_0}")] // redundant
    String(String),
    #[display("{_0}")] // redundant
    Number(Number),
}
```

It does not. Both spellings emit the same predicate, by two routes
that are forced to agree: with a template the bound pairs the field's
type with the trait the *placeholder* names, and without one it pairs
the same field's type with the trait the *derive* implements. Flagging
already requires those two traits to be equal, so wherever the rule
fires the bounds are equal too — `Number: Display` survives the fix,
and a `Number` that does not implement `Display` is still rejected at
the same place with the same error. A field whose type contains no
type parameter (`String` above) contributes no bound either way.

Nothing here needs an explicit `#[display(bound(...))]`; if one is
written anyway it is a separate attribute and the fix leaves it alone.

## Examples

### Tuple struct

**Avoid:**

```rust
#[derive(Display)]
#[display("{_0}")]
struct SanitizedHtml(String);
```

**Prefer:**

```rust
#[derive(Display)]
struct SanitizedHtml(String);
```

### Named single field, un-inlined argument

**Avoid:**

```rust
#[derive(Display)]
#[display("{}", message)]
struct Warning { message: String }
```

**Prefer:**

```rust
#[derive(Display)]
struct Warning { message: String }
```

### Single-field enum variant

**Avoid:**

```rust
#[derive(Display)]
enum ParseError {
    #[display("{_0}")]
    Io(std::io::Error),
    #[display("bad token at offset {_0}")]
    BadToken(usize),
}
```

**Prefer:**

```rust
#[derive(Display)]
enum ParseError {
    Io(std::io::Error),
    #[display("bad token at offset {_0}")]
    BadToken(usize),
}
```

### Enum-level `{_variant}`

**Avoid:**

```rust
#[derive(Display)]
#[display("{_variant}")]
enum Status {
    Idle,
    #[display("running for {_0}s")]
    Running(u64),
}
```

**Prefer:**

```rust
#[derive(Display)]
enum Status {
    Idle,
    #[display("running for {_0}s")]
    Running(u64),
}
```

### Not flagged

```rust
// `{}` forwards to `Display`, not to `LowerHex` — the attribute
// changes the rendering.
#[derive(LowerHex)]
#[lower_hex("{_0}")]
struct Mask(u32);

// `Debug` does not default to a forward.
#[derive(Debug)]
#[debug("{_0:?}")]
struct Payload(Vec<u8>);

// The width is applied here rather than passed through.
#[derive(Display)]
#[display("{_0:>8}")]
struct Padded(u32);

// Two fields: the template is mandatory.
#[derive(Display)]
#[display("{_0}")]
struct Pair(u32, u32);

// Deleting the variant attribute would fall back to `"unknown"`.
#[derive(Display)]
#[display("unknown")]
enum Opaque {
    #[display("{_0}")]
    Known(String),
}
```

## Configuration

None. Every part of the trigger is fixed by what `derive_more`
generates, so there is nothing for a project to tune: an attribute
either compiles to the derive's own default or it does not.

## Implementation notes

- **`LateLintPass` driven by `src/module_reparse.rs`.** The rule
  needs the written `#[derive(...)]` list to know which formatting
  trait is being implemented, and that attribute is consumed during
  macro expansion. Neither `EarlyLintPass` mode reaches it intact
  across every module — see
  [Reaching every module (source-layout rules)](./IMPLEMENTATION_CONVENTIONS.md#reaching-every-module-source-layout-rules).
  `perfectionist::clap_help_markdown`
  ([`src/rules/clap_help_markdown/collect.rs`](../src/rules/clap_help_markdown/collect.rs))
  is the reference implementation of this shape: it re-parses the
  crate's module files, walks the resulting ASTs for derive-bearing
  containers, and reads their helper attributes from the same parse.
  Guard descent into inline `mod { ... }` bodies with
  `live_module_spans`, and anchor each diagnostic through
  `enclosing_hir::find_enclosing_hir_ids` +
  `clippy_utils::diagnostics::span_lint_hir_and_then` so a local
  `#[allow]` / `#[expect]` resolves.

- **One diagnostic per attribute, not per container.** An enum whose
  variants are *all* single-field forwards carries one redundant
  attribute per variant, and every one of them is independently
  removable:

  ```rust
  #[derive(Display)]
  enum Value {
      #[display("{_0}")] String(String),
      #[display("{_0}")] Integer(i128),
      #[display("{_0}")] Float(f64),
  }
  ```

  A per-container diagnostic could not describe the mixed enum under
  *Examples* above, where only some variants qualify. Anchor each
  finding at its own variant rather than at the enum, so an `#[allow]`
  on one variant silences just that variant while an `#[allow]` on the
  enum still covers them all through the usual lint-level nesting.

- **Derive matching is by final path segment**, so `derive_more::Display`,
  a plain `Display` imported from `derive_more`, and a same-name
  re-export all match. A derive renamed through
  `use derive_more::Display as D;` is not caught — the same accepted
  limitation `perfectionist::unordered_derives`,
  `perfectionist::thiserror_usage`, and
  `perfectionist::clap_help_markdown` already carry.

- **Attribute matching is by bare name.** The formatting attributes
  are derive helper attributes, so they are only ever written
  unqualified: `#[derive_more::display(...)]` does not resolve. There
  is no qualified form to also accept.

- **`cfg_attr` unwrapping.** Both the derive list and the formatting
  attribute can be written as `#[cfg_attr(<cfg>, derive(Display))]` /
  `#[cfg_attr(<cfg>, display("{_0}"))]`. Reading them means
  descending one level into `cfg_attr`'s argument list, as
  `perfectionist::unordered_derives`
  ([`src/rules/unordered_derives.rs`](../src/rules/unordered_derives.rs))
  does for `derive`. A flagged attribute nested inside a `cfg_attr`
  whose predicate is not unconditionally true is a bail (the field
  count it was checked against may not hold under every
  configuration).

- **Parser style.** The placeholder scanner is parser-combinator-style
  `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#parser-style),
  not a regex. This rule is the first of the `derive_more` template
  cluster to need one, so factor it into a crate-internal module
  rather than keeping it private:
  [`uninlined-derive-more-args`](./uninlined-derive-more-args.md) and
  [`overly-long-derive-more-template`](./overly-long-derive-more-template.md)
  both name the same helper as shared infrastructure. The surface
  this rule needs is small — `take_escaped_brace` (`{{` / `}}`),
  `take_literal_text` (everything up to the next `{`), and
  `take_placeholder` (a `{...}` block, yielding its argument and its
  format spec) — and the two siblings need exactly the same three.

- **The whole attribute is deleted, not part of it.** A formatting
  attribute that passes the trigger holds a literal and at most one
  argument, and that argument is consumed by the single placeholder,
  so nothing in it survives the fix. `bound(...)` and
  `rename_all = "..."` cannot share an attribute with a template —
  `derive_more` parses them as alternatives — so they are always
  written as their own `#[display(bound(...))]` /
  `#[display(rename_all = "...")]` attributes and are untouched.

- **No proc-macro guard is needed.** The diagnostic span is the whole
  attribute, not a bare identifier inside it, so the built-in
  `report_in_external_macro: false` filter already covers this rule
  per the "vulnerable exactly when" test in
  [Suppressing proc-macro-synthesised violations](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).
  Record the omission as a short comment at the span-selection site
  so it reads as deliberate.

- **Version sensitivity.** The whole trigger is `derive_more`
  behaviour, and a lint has no practical way to read the resolved
  dependency's version from a late pass, so where a version boundary
  matters it has to be absorbed into the trigger itself:
  - **0.99 excludes itself.** Its attribute is
    `#[display(fmt = "...")]`, and there was no transparent
    delegation to be redundant with. Matching only the 1.0-onward
    `#[display("...", <expr>)]` shape — a leading string literal, not
    a `fmt =` key — rules it out for free, with no version check.
  - **The single-field trigger holds from 1.0 onward**, which is
    where transparent delegation arrived.
  - **The `{_variant}` trigger assumes 2.0**, which is where an
    enum-level template stopped being treated as transparent or
    wrapping by accident and a template without `{_variant}` became a
    default rather than a replacement. Take 2.0 as the baseline: it
    is the current major, and the shape at risk (an enum whose
    top-level template is exactly `{_variant}`) is rare. If that
    trade is unwanted, the `{_variant}` trigger is the piece to drop
    — everything else is version-robust from 1.0.

  The non-wrapping-enum bail-out needs no such call: 1.x ignored the
  variant's template there and 2.x lets it win, and declining to
  flag is correct under both.

### Difficulty

**Easy.** The predicate is a shallow syntactic match on one
attribute plus the container's field count, and the fix is a
deletion. The work that is not mechanical is the set of bail-outs —
the differing-trait cases, `Debug`, and the enum fallback — each of
which is a small, independently testable condition. Budget the UI
fixtures accordingly: the negative cases outnumber the positive ones,
and they are what the rule is actually made of.

## Default state

Active by default. The suggestion is output-preserving down to
format-spec propagation, the trigger has no per-project taste
component, and the rule stays silent on every shape where deletion
would change behaviour.

## Autofix

`MachineApplicable`. Delete the whole attribute, along with the line
it sits on. There is no partial-edit case to handle: an attribute
that reaches the fix holds nothing worth keeping, and the one shape
that would need surgery — a template nested inside a `cfg_attr` — is
already a bail.

## Non-goals

- **A unit variant restating its own name.** `#[display("Alpha")]` on
  a variant `Alpha` is also redundant, since a unit variant defaults
  to its name — but only when no `#[display(rename_all = "...")]` is
  in effect, and the check is a string comparison against the
  variant's identifier rather than a placeholder analysis. Distinct
  trigger, distinct failure mode; a separate rule.
- **Rewriting a non-redundant template.** Shortening, re-wrapping, or
  inlining a template that does real work belongs to the sibling
  rules below.

## Interaction with sibling rules

The `derive_more` template rules operate on disjoint parts of the
same attribute and compose in a fixed order:

- `redundant_derive_more_forward_template` (this rule) — the
  template does nothing; delete the attribute. Running first means
  the other two never spend a suggestion on an attribute that is
  about to disappear.
- [`uninlined-derive-more-args`](./uninlined-derive-more-args.md) —
  the template does something, but names its arguments the long way;
  inline them. Its rewrite can *create* this rule's trigger, by
  turning `#[display("{}", _0)]` into `#[display("{_0}")]`; both
  forms are flagged here already, so the two agree on the same
  containers either way.
- [`overly-long-derive-more-template`](./overly-long-derive-more-template.md)
  — the template does something and is too wide; fold it. A template
  short enough to be a lone placeholder never reaches its width
  threshold, so the two never fire on the same attribute.

[`manual-derive-more-impl`](./manual-derive-more-impl.md) is
upstream of all three: it converts a hand-written `impl Display` into
a derive plus an attribute, and the attribute it synthesises for a
single-field type is exactly the one this rule then flags.
[`error-type-derives`](./error-type-derives.md)'s `unused_display`
sub-check is orthogonal — it asks whether the `Display` derive should
be there at all, not what its template says.
