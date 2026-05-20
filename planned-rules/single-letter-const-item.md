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

struct Data<Left, Right, const M: usize, const N: usize> {
    left:  [Left;  N],
    right: [Right; N],
}

// Good
const DIMENSION_COUNT: usize = 2;
```

The rule fires on the declaration of the `const` item — not on the
use sites — and exactly mirrors the shape of
`single_letter_let_binding` but for items rather than locals. A
`let` binding tells the reader what a value computed; a `const`
item is a *named* compile-time constant that propagates through
type signatures, array lengths, and downstream documentation. The
information loss from `N` vs. `DIMENSION_COUNT` is at least as
large at the item level as at the `let` level, and often worse —
the item is referenced from anywhere in the module (or anywhere
in the crate, for `pub const`), so the reader doesn't have the
RHS of a nearby `let` to fall back on.

## Why restrict this?

This is a stylistic preference, not a correctness issue. Const
items spread further than `let` bindings — a `pub const` is read
from anywhere the item is in scope, and every reader has to map
the single letter back to its definition site. A descriptive
identifier carries its own documentation; `N`, `M`, `K` do not.

The rule is configurable rather than absolute because a small set
of single-letter constants do carry conventional meaning in
specific domains (`E` for Euler's number when defining
`const E: f64 = std::f64::consts::E;` for ergonomic reach,
`N`/`M` inside a fixture module that mirrors a paper's notation).
An allowlist exempts those cases without re-opening the door for
arbitrary single letters.

## What it covers

- `hir::ItemKind::Const` — free `const NAME: T = expr;`.
- `hir::ImplItemKind::Const` — associated `const NAME: T = expr;`
  inside an `impl` block (inherent or trait).
- `hir::TraitItemKind::Const` — associated const *declarations*
  inside a `trait` body (`const NAME: T;` with or without a
  default expression).
- `hir::StmtKind::Item` constants — block-level
  `const NAME: T = ...;` declared inside a function or `const`-eval
  context.

All four are the same syntactic shape (a `const` keyword followed
by a name); they are listed separately only because the HIR walks
them through different visitor hooks.

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
  `ItemKind::Const`, and the configuration shapes diverge
  (short-trait-impl exemption, idiomatic-name allowlist).
- **`const fn` declarations.** `const fn n() -> usize { 2 }` is a
  function with a single-letter name; that case belongs to a
  hypothetical `single_letter_function_name` rule, not here.
- **Const items inside `#[cfg(test)]` modules.** Test fixtures
  follow the same idiom that exempts `single_letter_let_binding`
  under `cfg(test)`; reuse `clippy_utils::is_in_test`.

## Configuration

Configure via `dylint.toml` under
`["perfectionist::single_letter_const_item"]`. Every field is
optional.

```toml
[perfectionist::single_letter_const_item]
extra_allowed_idents  = []   # default empty
ignore_allowed_idents = []   # default empty
```

### `extra_allowed_idents`: `[string]` (optional)

Additional identifiers allowed as `const`-item names, even outside
`#[cfg(test)]` code. Merged with the built-in defaults. Use this
for project-wide conventional names (e.g. `["E", "PI"]` in a
numerics-heavy crate that imports `std::f64::consts::*` constants
under shorter aliases).

The built-in default set is **empty**. The convention for `const`
items is SCREAMING_SNAKE_CASE descriptive identifiers; the cases
where a single capital letter is more readable than a word are
rare enough that the project lint baseline should not encode any
of them implicitly. (This is the deliberate divergence from
`single_letter_let_binding`'s built-in `["n"]`: `let n = …` for a
local count is well-attested; `const N: usize = …` for a
module-wide constant is not.)

### `ignore_allowed_idents`: `[string]` (optional)

Identifiers to drop from the exempt set, even if they appear in
`extra_allowed_idents` or in a future expansion of the built-in
defaults. Empty by default; checked after the merge, so this knob
always wins. Same shape as `single_letter_let_binding`.

## What to lint

For each visited `const` item:

1. Skip items inside external macros
   (`hir_in_external_macro`).
2. Skip items whose enclosing context returns true from
   `clippy_utils::is_in_test`.
3. Extract the item's identifier `Symbol`.
4. Require `is_single_ascii_letter(symbol.as_str())` (shared with
   the other `single_letter_*` rules; lives in
   `src/common.rs`).
5. Skip if the identifier is in the resolved `allowed_idents`
   set.
6. Emit `span_lint_and_help` on the identifier's span with the
   message
   `"const item `{ident}` has a single-letter name"` and the help
   `"rename to a descriptive identifier (e.g. `DIMENSION`,
   `BUFFER_LEN`, `MAX_RETRIES`)"`.

No autofix. Renaming a `const` item touches every reference; the
edit is large and `MachineApplicable` only with a
crate-wide rename that the lint pass cannot safely emit. A
diagnostic-only rule matches the existing
`single_letter_let_binding` shape (which also offers no autofix
despite locals being easier to rename than items).

## Implementation notes

- `LateLintPass`. The trigger does not need resolver state; an
  early pass would work too, but late keeps the rule consistent
  with the rest of the `single_letter_*` family.
- Use `check_item`, `check_impl_item`, `check_trait_item` for the
  three item-position cases. The block-level case
  (`StmtKind::Item`) is reached transitively by `check_item`
  because HIR lowers a function-body `const` into an `Item`
  attached to the enclosing body's HIR map.
- The `Symbol`-set configuration parsing reuses
  `resolve_symbol_set` from `single_letter_let_binding`. If the
  helper isn't already crate-internal, lift it to
  `src/common.rs` when implementing this rule — per CLAUDE.md's
  "factor it into `src/common.rs`" guidance for cross-rule
  helpers.

### Difficulty

**Easy.** The trigger is a four-line predicate over three HIR
node kinds; the configuration is a copy of
`single_letter_let_binding`'s with one default-set change.

## Default state

Active by default. Same justification as
`single_letter_let_binding`: the rule reflects the project's
baseline naming policy, and the empty allowlist means the rule
fires only on cases the project genuinely objects to.

## Interaction with sibling rules

- [`single-letter-const-generic`](./single-letter-const-generic.md)
  — covers the const-generic parameter shape
  (`<const N: usize>`). The two rules are disjoint at the trigger
  level (item vs. generic parameter); a single site cannot fire
  both.
- `perfectionist::single_letter_let_binding`
  (`src/rules/single_letter_let_binding.rs`) — the closest
  existing relative. Same configuration shape, different trigger
  position (item vs. local).
- `perfectionist::single_letter_generic`
  (`src/rules/single_letter_generic.rs`) — the type-parameter
  counterpart. Cited here only for completeness; the const-item
  rule has no overlap with type generics.
