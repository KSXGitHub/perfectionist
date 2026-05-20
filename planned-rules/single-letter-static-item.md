# `single_letter_static_item`

**Source:** sibling of `single_letter_const_item`, covering the
`static` item position that the const-item rule deliberately
scopes out.

## Statement

> A `static` item declared with a one-ASCII-letter name carries
> no information about what the static *is*.

```rust
// Bad
static N: AtomicUsize = AtomicUsize::new(0);
static D: u64 = 64;

// Good
static REQUEST_COUNT: AtomicUsize = AtomicUsize::new(0);
static DEFAULT_VALUE: u64 = 64;
```

The rule fires on the declaration of the `static` item — not on
its use sites.

## Why restrict this?

This is a stylistic preference, not a correctness issue. A
single-letter `static` item is opaque at every use site, and the
item's scope (module-wide or crate-wide for `pub static`) makes
that opacity propagate. A descriptive identifier carries its own
documentation. The `allowed_idents` knob exists for project-
specific conventional names; the default is empty.

## What it covers

- `hir::ItemKind::Static` — `static NAME: T = expr;` at any item
  position (free module-level, and the block-level form declared
  inside a function body, both reached through `check_item`).

## What it does *not* cover

- **`const` items.** Covered by the sibling
  [`single-letter-const-item`](./single-letter-const-item.md).
- **Const generic parameters.** Covered by
  [`single-letter-const-generic`](./single-letter-const-generic.md).
- **Foreign statics in `extern` blocks**
  (`extern { static N: c_int; }`). Different HIR node
  (`ForeignItemKind::Static`).

## Configuration

```toml
[single_letter_static_item]
allowed_idents = []   # default empty
```

### `allowed_idents`: `[string]` (optional)

Identifiers the rule will not flag. Empty by default. Example:

```toml
[single_letter_static_item]
allowed_idents = ["X"]
```

## What to lint

For each visited `static` item:

1. Skip items inside external macros
   (`hir_in_external_macro`).
2. Extract the item's identifier `Symbol`.
3. Require `is_single_ascii_letter(symbol.as_str())` (shared with
   the other `single_letter_*` rules; lives in
   `src/common.rs`).
4. Skip if the identifier is in the configured `allowed_idents`
   set.
5. Emit `span_lint_and_help` on the identifier's span with the
   message ``"static item `{ident}` has a single-letter name"`` and
   the help
   ``"rename to a descriptive identifier (e.g. `BUFFER`, `CACHE`, `COUNTER`)"``.

No autofix. Renaming a `static` item touches every reference; the
edit is large and `MachineApplicable` only with a crate-wide
rename that the lint pass cannot safely emit.

## Implementation notes

- `allowed_idents` parses straight into a `BTreeSet<Symbol>`.

### Difficulty

**Easy.** The trigger is a four-step predicate over one HIR node
kind; the configuration is a single `BTreeSet<Symbol>`.

## Default state

Active by default. Empty `allowed_idents`.

## Interaction with sibling rules

- [`single-letter-const-item`](./single-letter-const-item.md) —
  the const-item counterpart. Disjoint trigger
  (`ItemKind::Static` vs. `ItemKind::Const`); a single site
  cannot fire both.
