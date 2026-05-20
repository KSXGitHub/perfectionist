# `single_letter_const_item`

**Source:** sibling of the four existing `single_letter_*` rules
(`single_letter_generic`, `single_letter_let_binding`,
`single_letter_function_param`, `single_letter_closure_param`),
extended to cover named `const` items — both free and associated.

## Statement

> A `const` item declared with a one-ASCII-letter name carries no
> information about what the constant *is*.

```rust
// Bad
const N: usize = 2;

impl Buffer {
    const D: usize = 64;
}

// Good
const DIMENSION_COUNT: usize = 2;

impl Buffer {
    const DEFAULT_CAPACITY: usize = 64;
}
```

The rule fires on the declaration of the `const` item — not on
its use sites. The trigger position is the item analogue of
`single_letter_let_binding`'s local-binding trigger; the
configuration shape differs (see *Interaction with sibling rules*
below).

## Why restrict this?

This is a stylistic preference, not a correctness issue. A
single-letter `const` item is opaque at every use site, and the
item's scope (module-wide or crate-wide for `pub const`) makes
that opacity propagate further than a `let` binding's would. A
descriptive identifier carries its own documentation. The
`allowed_idents` knob exists for project-specific conventional
names; the default is empty.

## What it covers

- `hir::ItemKind::Const` — free `const NAME: T = expr;`, including
  the block-level form (`const NAME: T = ...;` declared inside a
  function body), which the HIR lowers to a nested `Item` reached
  through the same `check_item` hook.
- `hir::ImplItemKind::Const` — associated `const NAME: T = expr;`
  inside an `impl` block (inherent or trait).
- `hir::TraitItemKind::Const` — associated const *declarations*
  inside a `trait` body (`const NAME: T;` with or without a
  default expression).

## What it does *not* cover

- **`static` items.** A `static N: AtomicUsize = ...;` is a
  distinct construct with distinct conventions (interior
  mutability, address stability). If a `single_letter_static`
  rule is wanted later it should be a sibling; do not extend this
  one. The two rules' configuration knobs would be the same
  shape, but bundling them obscures which one a reader needs to
  silence at a given site.
- **Const generic parameters** (`<const N: usize>`). Covered by
  the sibling [`single-letter-const-generic`](./single-letter-const-generic.md)
  rule. The trigger lives on `GenericParamKind::Const`, not on
  `ItemKind::Const`.

## Configuration

```toml
[single_letter_const_item]
allowed_idents = []   # default empty
```

### `allowed_idents`: `[string]` (optional)

Identifiers the rule will not flag. Empty by default. Example:

```toml
[single_letter_const_item]
allowed_idents = ["X"]
```

## What to lint

For each visited `const` item:

1. Skip items inside external macros
   (`hir_in_external_macro`).
2. Extract the item's identifier `Symbol`.
3. Require `is_single_ascii_letter(symbol.as_str())` (shared with
   the other `single_letter_*` rules; lives in
   `src/common.rs`).
4. Skip if the identifier is in the configured `allowed_idents`
   set.
5. Emit `span_lint_and_help` on the identifier's span with the
   message ``"const item `{ident}` has a single-letter name"`` and
   the help
   ``"rename to a descriptive identifier (e.g. `DIMENSION`, `BUFFER_LEN`, `MAX_RETRIES`)"``.

No autofix. Renaming a `const` item touches every reference; the
edit is large and `MachineApplicable` only with a
crate-wide rename that the lint pass cannot safely emit. A
diagnostic-only rule matches the existing
`single_letter_let_binding` shape (which also offers no autofix
despite locals being easier to rename than items).

## Implementation notes

- Use `LateLintPass` for consistency with the rest of the
  `single_letter_*` family. The rule itself doesn't need
  `TyCtxt` (no `is_in_test`, no resolver queries), so
  `EarlyLintPass` would also work; consistency is the only reason
  to pick late.
- Hook up the three `check_*` callbacks listed in
  [What it covers](#what-it-covers).
- `allowed_idents` parses straight into a `BTreeSet<Symbol>`. No
  reuse of `resolve_symbol_set` (the helper
  `single_letter_let_binding` uses for its `extra_*` /
  `ignore_*` pair) — without built-in defaults there is nothing
  to subtract from, so the single-field shape doesn't need it.

### Difficulty

**Easy.** The trigger is a four-step predicate over three HIR
node kinds; the configuration is a single `BTreeSet<Symbol>`.

## Default state

Active by default. Same justification as
`single_letter_let_binding`: the rule reflects the project's
baseline naming policy, and the empty exempt set means the rule
fires only on cases the project genuinely objects to.

## Interaction with sibling rules

- [`single-letter-const-generic`](./single-letter-const-generic.md)
  — covers the const-generic parameter shape
  (`<const N: usize>`). The two rules are disjoint at the trigger
  level (item vs. generic parameter); a single site cannot fire
  both.
- `perfectionist::single_letter_let_binding`
  (`src/rules/single_letter_let_binding.rs`) — the closest
  existing relative. Different trigger position (item vs. local);
  simpler configuration shape (single `allowed_idents` field
  rather than let_binding's `extra_allowed_idents` /
  `ignore_allowed_idents` pair, because this rule has no built-in
  defaults to subtract from).
- `perfectionist::single_letter_generic`
  (`src/rules/single_letter_generic.rs`) — the type-parameter
  counterpart. Cited here only for completeness; the const-item
  rule has no overlap with type generics.
