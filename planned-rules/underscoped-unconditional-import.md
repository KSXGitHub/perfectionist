# `underscoped_unconditional_import`

**Source:** project convention. The dual of
[`overscoped-conditional-import`](./overscoped-conditional-import.md):
together they encode one placement policy — *unconditional* imports
belong at module scope, *conditional* (`#[cfg(...)]`) imports belong
at the narrowest scope that uses them. This file covers the
unconditional half. Function-local `use` statements accrete as code
is pasted between functions and as AI assistants emit
self-contained snippets that re-import what the module already has;
when the import carries no `#[cfg]` and would not collide at module
scope, its function-local placement is needless narrowing.

## Statement

A plain `use ...;` (no `#[cfg(...)]`) declared **inside a function
body** whose imported item *could* sit at module scope instead — i.e.
hoisting it would not introduce a name collision — is scoped more
narrowly than it needs to be. The import belongs at module scope,
where it is written once and shared, rather than buried in (and
possibly duplicated across) function bodies.

```rust
// Avoid — a plain import scoped to one function for no reason.
fn parse(s: &str) -> Result<Config> {
    use serde_json::from_str;
    from_str(s).map_err(Into::into)
}
```

```rust
// Prefer — the import sits at module scope.
use serde_json::from_str;

fn parse(s: &str) -> Result<Config> {
    from_str(s).map_err(Into::into)
}
```

Both compile identically. The choice is where the import lives, and
the policy puts unconditional imports at module scope.

## Why restrict this?

This is a stylistic preference, not a correctness issue: a
function-local `use` compiles and behaves identically. The preference
is for a single, predictable home for unconditional imports:

- **One canonical place to read the module's dependencies.** When
  every un-gated import sits in the module's import block, a reader
  learns what the module pulls in by scanning one place. Imports
  sprinkled through function bodies hide that surface and make "does
  this module already import `X`?" a whole-file search.

- **It prevents silent duplication.** The same `use foo::Bar;`
  repeated at the top of three functions is three lines that must be
  kept in sync; hoisted once to module scope it is one line. The
  function-local form invites exactly this drift as functions are
  copied.

- **Module scope is the conventional home for an unconditional
  import.** A function-local `use` reads as a signal — "this import is
  here for a reason, probably to avoid a collision." When there is no
  such reason, the placement misleads: it implies a constraint that
  does not exist.

The conditional case points the other way — a `#[cfg(...)]` import
used only inside function bodies is *better* sunk into the gated scope
that uses it (a copy per gated function or branch), so that the
conditional-compilation surface stays minimal and co-located. That
direction is
[`overscoped-conditional-import`](./overscoped-conditional-import.md).
The two rules never disagree on a single `use`: this one fires only
on un-gated imports, its dual only on `#[cfg(...)]`-gated ones. A
genuinely collision-driven function-local import is the exemption
that keeps this rule honest (see below), so when the rule *does* fire
the "this import is here for a reason" signal was indeed spurious.

## What to lint

For every named leaf imported by a `use` item declared **inside a
function body** (a free `fn`, an inherent/trait-impl method, or a
nested block within one), flag the leaf when **all** of:

1. its `use` item carries **no** `#[cfg(...)]` gate (a conditional
   function-local import is the *desired* state under
   [`overscoped-conditional-import`](./overscoped-conditional-import.md),
   never a violation here); and
2. it is a **named leaf** (`Leaf` / `Leaf as R`), not a glob (`*`) —
   see *What's not in scope*. A brace-list import (`use a::b::{A, B};`)
   is **not** exempt: each of its leaves is evaluated independently
   against the conditions here, exactly as if it were written as a
   separate single import; and
3. hoisting it to the **enclosing module's** scope would **not**
   collide: the module scope (and the scopes of sibling functions
   whose identical imports would merge into the same hoisted line)
   does not already bind that name to a different item, and there is
   no other function-local `use` of the same leaf name resolving to a
   *different* item anywhere in the module; and
4. the leaf's resolved path is **not** on the collision-prone
   keep-local list (see *Configuration*).

When it fires, the autofix removes the hoistable leaf from the
function-local `use` and adds the equivalent import at module scope
(merging identical hoists from sibling functions into one line, and
coordinating with the import granularity / grouping rules for reflow).
For a brace list, only the qualifying leaves are lifted out; any leaf
that fails a condition stays behind, splitting the list. The fix is
`MachineApplicable` only when the no-collision determination is
certain.

### What's *not* in scope

- **`#[cfg(...)]`-gated function-local imports.** Out of scope by
  condition (1); they are the end state the dual rule pushes toward.

- **Imports that would collide if hoisted** — the headline exemption.
  Two functions in the same module that each do `use ...::Write;` for
  *different* `Write` traits cannot both hoist to module scope; the
  function-local placement is load-bearing. Likewise an import whose
  name already names a different item at module scope. Detected by
  condition (3): when hoisting would shadow or clash, the rule stays
  silent.

  ```rust
  // Not flagged — hoisting either `Write` would collide with the other.
  fn render(buf: &mut String) -> fmt::Result {
      use std::fmt::Write;
      write!(buf, "{}", 1)
  }
  fn emit(out: &mut Vec<u8>) -> io::Result<()> {
      use std::io::Write;
      write!(out, "{}", 1)
  }
  ```

- **Well-known collision-prone paths** — a configurable default list
  (condition (4)). Even when *this* module has no actual second
  `Write` in scope, a handful of paths are conventionally kept
  function-local because hoisting them is collision-prone or
  ambiguity-inviting as the module grows. The canonical pair is
  `core::fmt::Write` vs. `std::io::Write` (and `std::fmt::Write` vs.
  `std::io::Write`); the default set also includes the analogous
  `fmt::Result` vs. `io::Result` pair. The list is tunable via the
  `extra` / `ignore` knobs (see *Configuration*).

- **Glob imports** (`use foo::*;`). A glob has no single leaf to
  re-point and changes name resolution wholesale when hoisted, so it
  is out of scope.

  **Brace-list imports** (`use foo::{A, B};`) are **not** exempt: a
  brace list is just shorthand for several single imports sharing a
  prefix, and each leaf is independently hoistable. The rule evaluates
  each leaf on its own and lifts out the hoistable ones, splitting the
  list when only some qualify:

  ```rust
  // Avoid — both leaves are hoistable; the brace bundling does not
  // make them function-local for any reason.
  fn parse(s: &str) -> Result<Config> {
      use serde_json::{from_str, Value};
      let v: Value = from_str(s)?;
      Config::from_value(v)
  }
  ```
  ```rust
  // Prefer
  use serde_json::{from_str, Value};

  fn parse(s: &str) -> Result<Config> {
      let v: Value = from_str(s)?;
      Config::from_value(v)
  }
  ```

- **Imports already at module scope.** Those are the desired end
  state.

## Configuration

```toml
# dylint.toml
#
# Inactive by default. Enable in `[perfectionist].enable`. The rule has
# a single direction (hoist a collision-free unconditional import to
# module scope), so there is no `style` knob.
[perfectionist::underscoped_unconditional_import]

# Paths kept function-local even when no actual collision exists in
# the module, because hoisting them is collision-prone or ambiguity-
# inviting. The default set is built in; the two knobs below extend
# and trim it, following the `extra` / `ignore` paradigm used by
# `perfectionist::macro_trailing_comma` (`extra_macros` / `ignore`) and
# `perfectionist::impure_macro_arguments`
# (`extra_pure_methods` / `ignore_pure_methods`).
#
# Built-in default (illustrative — settle the exact set in code):
#   ::core::fmt::Write
#   ::std::fmt::Write
#   ::std::io::Write
#   ::core::fmt::Result
#   ::std::io::Result
#
# Entries are path strings matched against the import's resolved path,
# subject to the leading-`::` absolute-vs-relative convention in
# IMPLEMENTATION_CONVENTIONS.md ("Path-shaped config values"); reuse
# `src/abs_path.rs` for the matching.

# Additional paths to keep function-local (added to the built-in set).
extra_keep_local = []

# Built-in keep-local paths to drop, allowing them to be hoisted like
# any other import.
ignore_keep_local = []
```

The collision-prone list is the only knob: actual collisions are
detected structurally (condition (3)) and need no configuration. A
project that wants the rule off globally leaves it out of
`[perfectionist].enable`; a one-off intentional function-local import
is suppressed with
`#[allow(perfectionist::underscoped_unconditional_import)]`.

## Implementation notes

These notes fix the *shape* of the rule and deliberately stop short
of naming specific `rustc` / `clippy_utils` APIs; settle those
against the compiler during implementation.

- **A split source-layout rule: re-parse for candidates, late HIR for
  collisions.** Two facts the rule needs are not both available in one
  view. Whether a function-local `use` carries a `#[cfg]` is a
  *written-source* fact — an active `#[cfg]` is stripped from the HIR
  during expansion, so HIR alone cannot tell an un-gated import
  (condition 1) from a gated one. Reaching every function in every
  separate-file `mod foo;` with the written layout is the
  source-layout problem documented in
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#reaching-every-module-source-layout-rules):
  run as a **`LateLintPass`** and re-parse the crate's module files
  via `src/module_reparse.rs` (guarding inline-module descent with
  `live_module_spans`) to enumerate candidate function-local imports,
  decomposing each brace list into its leaves and skipping globs.
  Whether hoisting *collides*
  (condition 3), on the other hand, is a *name-resolution* fact
  available only with `TyCtxt`. So park each candidate as a
  `PendingViolation` (the `queue` submodule pattern from
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md))
  and resolve the collision question in the late pass, anchoring the
  diagnostic at the enclosing HIR node via
  `enclosing_hir::find_enclosing_hir_ids` so a per-item `#[allow]` /
  `#[expect]` resolves.

- **Collision detection must be conservative.** Unlike its dual,
  this rule *widens* an import's scope, which *can* introduce a
  collision — so resolution, not a syntactic scan, is mandatory here.
  Ask the resolver whether the leaf name is already bound at the
  target module scope to a different item, and whether any sibling
  function binds the same leaf name to a different item (those would
  collide once both are hoisted/merged). When the answer is uncertain,
  do **not** fire: a wrong hoist breaks compilation, so bias toward
  silence exactly as
  [`path-qualification-mismatch`](./path-qualification-mismatch.md)
  and [`private-reexport-imports`](./private-reexport-imports.md) do
  for their import-rewriting fixes.

- **Keep-local list matching.** Match the import's *resolved* path
  against the built-in-plus-`extra_keep_local`-minus-`ignore_keep_local`
  set. Parse the path-string entries with parser-combinator `take_*`
  functions and honour the leading-`::` absolute-vs-relative
  convention, reusing `src/abs_path.rs`'s `canonical_key` /
  `validate_absolute` rather than re-deriving path matching — see
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#path-shaped-config-values-the-leading--convention).

- **Autofix and the import rules.** Emit one module-scope `use` per
  hoisted leaf; `perfectionist::import_granularity_mismatch` and
  `perfectionist::import_grouping_mismatch` reflow the result on a
  later `--fix` pass. When several sibling functions import the same
  leaf, the fix removes each function-local copy and adds a single
  module-scope line. Preserve any `as` rename.

- **Proc-macro suppression.** If the diagnostic span is narrower than
  the whole `use` item, apply the standard guard and add a
  `ui/underscoped_unconditional_import_proc_macro.rs` fixture per the
  "Suppressing proc-macro-synthesised violations" section of
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).
  Anchoring on the whole `use` item likely makes the rule a
  non-participant; record that reasoning at the span-selection site.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions, in particular the `perfectionist::*`
  lint-name namespacing every registered lint follows.

### Difficulty

**Hard.** The split between a re-parse pass (to see `#[cfg]` absence
and reach every function) and a late resolution pass (to prove a
hoist is collision-free) is the same shape as the existing
source-layout import rules, but the collision proof — across the
module scope *and* every sibling function's would-be-hoisted import,
conservatively enough to keep false positives at zero — is the wedge.

## Interaction with sibling rules

- [`overscoped-conditional-import`](./overscoped-conditional-import.md)
  is the exact dual and the reason this rule is scoped to un-gated
  imports only: the two partition every single named `use` by the
  presence of a `cfg` gate, so they can both be active without ever
  firing on the same import or fighting over a fix.
- `perfectionist::import_granularity_mismatch`
  ([`src/rules/import_granularity_mismatch.rs`](../src/rules/import_granularity_mismatch.rs))
  and `perfectionist::import_grouping_mismatch`
  ([`src/rules/import_grouping_mismatch.rs`](../src/rules/import_grouping_mismatch.rs))
  reflow the module-scope import block after this rule hoists a line
  into it; this rule decides *which scope* an import lives in, they
  decide how the resulting block is shaped.
- [`path-qualification-mismatch`](./path-qualification-mismatch.md)
  governs the orthogonal axis of *whether* to import at all (path vs.
  `use`); this rule, given that there is a `use`, governs which scope
  it sits in. They can both be active without conflict.

## Interaction with stock lints

No Clippy or rustc lint covers this. `clippy::items_after_statements`
is the nearest neighbour and is unrelated: it flags an item declared
*after a statement within a block* (an ordering concern inside one
scope), not an import that should move *out* of the function to module
scope. Rustc's `unused_imports` only retires imports that are never
used; the function-local import here *is* used. Nothing in Clippy
relates an import's declaration scope to where it could live given the
names in scope. This rule fills that gap.

## Default state

Inactive by default. "Unconditional imports belong at module scope,
hoist the collision-free ones" is an opinionated stance — a
function-local `use` is frequently a deliberate choice, and flagging
all hoistable ones project-wide is presumptuous — so the rule ships
no baseline and is opted into via `[perfectionist].enable`, alongside
its dual [`overscoped-conditional-import`](./overscoped-conditional-import.md).
It expresses a single fixed direction rather than a choice, so it has
no mandatory `style` value.
