# `single_letter_const_item`

**Source:** sibling of the four existing `single_letter_*` rules
(`single_letter_generic`, `single_letter_let_binding`,
`single_letter_function_param`, `single_letter_closure_param`),
covering the `const`-item position those rules scope out.

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
its use sites.

## Why restrict this?

This is a stylistic preference, not a correctness issue. A
single-letter `const` item is opaque at every use site, and the
item's scope (module-wide or crate-wide for `pub const`) makes
that opacity propagate. A descriptive identifier carries its own
documentation. The `allowed_idents` knob exists for
project-specific conventional names; the default is empty.

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

- **`static` items.** Covered by the sibling
  [`single-letter-static-item`](./single-letter-static-item.md)
  rule.
- **Const generic parameters** (`<const N: usize>`). Covered by
  the sibling [`single-letter-const-generic`](./single-letter-const-generic.md)
  rule.

## Configuration

```toml
[single_letter_const_item]
allowed_idents = []   # default empty
```

### `allowed_idents`: `[single-character string]` (optional)

Identifiers the rule will not flag. Each entry is a single ASCII
letter (deserialised as `char`, rejected with a config-parse
error otherwise — a typo like `["xy"]` or `["1"]` does not pass
through silently). Empty by default. Example:

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
edit is large and `MachineApplicable` only with a crate-wide
rename that the lint pass cannot safely emit.

## Implementation notes

- `allowed_idents` deserialises as `Vec<AsciiLetter>` (the shared
  newtype in `src/ascii_letter.rs` whose `TryFrom<char>` impl
  rejects any non-ASCII-letter entry at config-parse time) and is
  interned into a `BTreeSet<Symbol>`.

### Difficulty

**Easy.** The trigger is a four-step predicate over three HIR
node kinds; the configuration is a single `BTreeSet<Symbol>`.

## Default state

Active by default. Empty `allowed_idents`.

## Interaction with sibling rules

- [`single-letter-const-generic`](./single-letter-const-generic.md)
  — the const-generic counterpart. Disjoint trigger
  (`ItemKind::Const` vs. `GenericParamKind::Const`); a single
  site cannot fire both.
- [`single-letter-static-item`](./single-letter-static-item.md)
  — the static-item counterpart. Disjoint trigger
  (`ItemKind::Const` vs. `ItemKind::Static`).
