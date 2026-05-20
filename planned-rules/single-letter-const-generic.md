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

This is the const-parameter counterpart of
`perfectionist::single_letter_generic`. The same arguments apply:
a single-letter parameter forces every reader downstream of the
declaration to scroll back to recover its role, and in a long
`impl` block the cost is large; in a short one it is small enough
that the canonical `impl<const N: usize> Foo for [T; N]` shape
remains readable. The rule reuses the existing
`single_letter_generic` exemption mechanism — short trait `impl`
blocks — and adds the same `extra_allowed_idents` /
`ignore_allowed_idents` knob shape that `single_letter_let_binding`
uses, so projects can opt single letters into the exempt set per
their own conventions.

## Why restrict this?

This is a stylistic preference, not a correctness issue. The same
"readers must keep the parameter's role in head while reading the
type signature" argument that motivates
`single_letter_generic` applies here. Const generics do carry
some idiomatic weight — `N` and `M` for array dimensions, matrix
sizes, and slice lengths appear in rustc's own examples — but
that weight is not so settled that the rule should ship a default
exempt set; projects that want to keep those names add them
locally via `extra_allowed_idents`.

## What it covers

`hir::GenericParamKind::Const { .. }` — the const-generic
declaration syntax `<const NAME: T>`, anywhere generics are
syntactically allowed. The trigger is positional and does not
filter by enclosing item kind.

## What it does *not* cover

- **Type generic parameters** (`<T>`, `<K, V>`). Covered by the
  existing `perfectionist::single_letter_generic` lint at
  `src/rules/single_letter_generic.rs`. The two rules are
  disjoint at the trigger level and share only the short-trait-
  impl exemption helper.
- **Lifetime parameters** (`<'a>`, `<'de>`). Single-letter
  lifetime names are the universal Rust idiom; a lint that
  flagged them would fire on essentially every lifetime in the
  ecosystem. Out of scope here and not planned.
- **`const NAME: T = ...;` items.** Covered by
  [`single-letter-const-item`](./single-letter-const-item.md).

## Configuration

```toml
[single_letter_const_generic]
short_impl_max_lines  = 20            # mirrors `single_letter_generic`
extra_allowed_idents  = []            # default empty
ignore_allowed_idents = []            # default empty
```

### `short_impl_max_lines`: `unsigned integer` (optional)

Maximum number of source lines an `impl Trait for Type` block
may span and still permit single-letter const generic parameter
names. Defaults to `20`. The semantics are identical to the
existing `single_letter_generic` knob — same exemption, applied to
const generic parameters instead of type parameters.

### `extra_allowed_idents`: `[string]` (optional)

Additional identifiers added to the exempt set. Empty by default
— the rule ships no built-in exempt single letters for const
generics. A project that wants to keep the array-dimension idiom
adds the names explicitly:

```toml
[single_letter_const_generic]
extra_allowed_idents = ["N", "M"]
```

### `ignore_allowed_idents`: `[string]` (optional)

Identifiers to drop from the exempt set, even if they appear in
`extra_allowed_idents`. Empty by default; checked after the
merge, so this knob always wins. Same shape as
`single_letter_let_binding` and `single_letter_const_item`.

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
6. Skip if the identifier is in the resolved `allowed_idents`
   set.
7. Skip if the enclosing item is a trait `impl` block whose span
   covers `≤ short_impl_max_lines` lines (shared with
   `single_letter_generic`).
8. Emit `span_lint_and_help` on the parameter's span with the
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

- `LateLintPass` is required: the short-trait-impl exemption
  walks `tcx.hir_parent_iter(...)` and queries
  `sess().source_map()` for the line span (see
  `src/rules/single_letter_generic.rs`), neither of which is
  available in early context. Use the `check_generic_param` hook.
- Share the short-trait-impl helper with `single_letter_generic`
  rather than re-implementing it. On the first PR to add this
  rule, lift `enclosing_short_trait_impl` and `span_line_count`
  out of `src/rules/single_letter_generic.rs` into a crate-
  internal module — `src/common.rs` if they remain trivial, or a
  dedicated `src/short_impl.rs` if either grows its own
  invariants. The existing rule's call site updates to import the
  helper from the new home.
- The `Symbol`-set configuration parsing reuses
  `resolve_symbol_set` (see `single_letter_let_binding`); if it
  isn't already in `src/common.rs`, lift it there at the same
  time as the short-impl helper.

### Difficulty

**Easy.** The trigger is a three-line predicate over a single
`GenericParam` visitor hook. The configuration replays
`single_letter_generic`'s `short_impl_max_lines` plus
`single_letter_let_binding`'s `extra_allowed_idents` /
`ignore_allowed_idents` knobs; both shapes already exist in the
codebase. The only non-trivial part is the helper hoist, which is
mechanical.

## Default state

Active by default with an empty exempt set. Projects that want to
keep `N` / `M` as const generic names opt in via
`extra_allowed_idents`.

## Interaction with sibling rules

- `perfectionist::single_letter_generic`
  (`src/rules/single_letter_generic.rs`) — the type-parameter
  counterpart. Disjoint trigger (`GenericParamKind::Type` vs.
  `::Const`), shared short-trait-impl helper. A single
  declaration with both type and const parameters can produce one
  diagnostic per offending parameter, one from each rule — that
  is the intended behaviour, not an interaction bug.
- [`single-letter-const-item`](./single-letter-const-item.md) —
  the const-item counterpart. Disjoint trigger
  (`ItemKind::Const` vs. `GenericParamKind::Const`); the two
  rules can fire on adjacent lines of source but never on the
  same node.
- `perfectionist::single_letter_let_binding`
  (`src/rules/single_letter_let_binding.rs`) — cited here for
  configuration shape only; the `extra_allowed_idents` /
  `ignore_allowed_idents` knobs work the same way.
