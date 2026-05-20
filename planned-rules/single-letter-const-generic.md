# `single_letter_const_generic`

**Source:** sibling of `perfectionist::single_letter_generic`
(`src/rules/single_letter_generic.rs`), extended to cover const
generic parameters, which the existing rule deliberately scopes
out by early-returning on every `GenericParamKind` other than
`Type`.

## Statement

> A const generic parameter declared with a one-ASCII-letter name
> propagates through the type signature the same way a
> single-letter type parameter does.

```rust
// Bad
struct Data<Left, Right, const M: usize, const N: usize> {
    left:  [Left;  M],
    right: [Right; N],
}

// Good
struct Data<Left, Right, const LEFT_LEN: usize, const RIGHT_LEN: usize> {
    left:  [Left;  LEFT_LEN],
    right: [Right; RIGHT_LEN],
}
```

## Why restrict this?

This is a stylistic preference, not a correctness issue. A
single-letter const generic parameter is opaque at every use
site; a descriptive identifier documents the parameter's role.

## What it covers

`hir::GenericParamKind::Const { .. }` — the const-generic
declaration syntax `<const NAME: T>`, anywhere generics are
syntactically allowed. The trigger is positional and does not
filter by enclosing item kind.

## What it does *not* cover

- **Type generic parameters** (`<T>`, `<K, V>`). Covered by the
  existing `perfectionist::single_letter_generic` lint at
  `src/rules/single_letter_generic.rs`. Disjoint trigger.
- **Lifetime parameters** (`<'a>`, `<'de>`). Not
  `GenericParamKind::Const`.
- **`const NAME: T = ...;` items.** Covered by
  [`single-letter-const-item`](./single-letter-const-item.md).

## Configuration

```toml
[single_letter_const_generic]
allowed_idents = []   # default empty
```

### `allowed_idents`: `[string]` (optional)

Identifiers the rule will not flag. Empty by default. Example:

```toml
[single_letter_const_generic]
allowed_idents = ["X"]
```

## What to lint

For each visited generic parameter:

1. Match `param.kind` against `hir::GenericParamKind::Const`;
   reject every other kind (this rule does *not* fire on
   `Type` or `Lifetime` — those are covered by
   `single_letter_generic` and out-of-scope respectively).
2. Skip synthetic parameters.
3. Skip parameters inside external macros
   (`hir_in_external_macro`).
4. Extract the parameter's identifier `Symbol`.
5. Require `is_single_ascii_letter(symbol.as_str())` (shared
   helper in `src/common.rs`).
6. Skip if the identifier is in the configured `allowed_idents`
   set.
7. Emit `span_lint_and_help` on the parameter's span with the
   message ``"const generic parameter `{ident}` has a single-letter name"``
   and the help
   ``"rename to a descriptive identifier (e.g. `LEN`, `COLS`, `LANES`)"``.

No autofix. Renaming a const generic parameter rewrites every
reference in the parameter's type, where clause, body, and every
generic argument supplied at every call site — the edit is
project-wide and not safely `MachineApplicable` from a single
declaration. Diagnostic-only matches
`single_letter_generic`'s behaviour for the same reason.

## Implementation notes

- Use `LateLintPass` with the `check_generic_param` hook.
- `allowed_idents` parses straight into a `BTreeSet<Symbol>`.

### Difficulty

**Easy.** The trigger is a six-step predicate over a single
`GenericParam` visitor hook.

## Default state

Active by default. Empty `allowed_idents`.

## Interaction with sibling rules

- `perfectionist::single_letter_generic`
  (`src/rules/single_letter_generic.rs`) — the type-parameter
  counterpart. Disjoint trigger (`GenericParamKind::Type` vs.
  `::Const`). A single declaration with both type and const
  parameters can produce one diagnostic per offending parameter,
  one from each rule — that is the intended behaviour, not an
  interaction bug.
- [`single-letter-const-item`](./single-letter-const-item.md) —
  the const-item counterpart. Disjoint trigger
  (`ItemKind::Const` vs. `GenericParamKind::Const`); the two
  rules can fire on adjacent lines of source but never on the
  same node.
- `perfectionist::single_letter_let_binding`
  (`src/rules/single_letter_let_binding.rs`) — cited for context.
  This rule's `allowed_idents` is the simplified single-field
  version of let_binding's `extra_allowed_idents` /
  `ignore_allowed_idents` pair; no built-in defaults here means
  no need for the subtract-from-defaults knob.
