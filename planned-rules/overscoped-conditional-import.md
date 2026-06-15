# `overscoped_conditional_import`

**Source:** project convention. The dual of
[`underscoped-unconditional-import`](./underscoped-unconditional-import.md):
together they encode one placement policy — *unconditional* imports
belong at module scope, *conditional* (`#[cfg(...)]`) imports belong
at the narrowest scope that uses them. This file covers the
conditional half. AI assistants and LSP "add import" actions
reliably drop every new `use` at the top of the file, including a
`#[cfg(test)] use ...;` that only one test helper touches, leaving
the module header carrying a conditional import its body does not
need module-wide.

## Statement

A `#[cfg(...)] use ...;` declared at **module scope** whose imported
item is used **only inside `#[cfg(...)]`-gated functions** is scoped
more broadly than it needs to be: the conditional import — and the
conditional-compilation surface it adds to the module header — could
move into each function that uses it, riding along with that
function's own `#[cfg]` instead of adding a separate gate at module
scope.

```rust
// Avoid — module-level conditional import; the only user is itself
// a #[cfg(unix)]-gated function.
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
fn set_executable(p: &Path) -> io::Result<()> {
    let mut perms = fs::metadata(p)?.permissions();
    perms.set_mode(0o755);   // the only use of PermissionsExt
    fs::set_permissions(p, perms)?;
    Ok(())
}
```

```rust
// Prefer — the conditional import travels inside the gated function,
// which already carries the #[cfg(unix)] gate.
#[cfg(unix)]
fn set_executable(p: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(p)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(p, perms)?;
    Ok(())
}
```

When more than one `#[cfg]`-gated function uses the item, the
preferred form puts a `use` inside **each** of them — a copy per
gated function is preferred over one shared module-level conditional
import. Both forms compile identically under every feature
combination; the choice is where the conditional import lives.

## Why restrict this?

This is a stylistic preference, not a correctness issue: the
module-level form compiles and behaves identically. The preference
is about keeping conditional-compilation surface minimal and
co-located:

- **A module-level `#[cfg(...)] use` is conditional-compilation
  surface in the module header.** Every reader scanning the import
  block must now reason about which feature combinations the line is
  live under, even though the conditionality is relevant only inside
  the gated functions that use it. Sinking the import into each of
  those functions confines the `#[cfg]` to the places it matters.

- **It removes a duplicated gate.** When every use sits inside a
  `#[cfg(...)]`-gated function, an import gated by the *same* predicate
  up at module scope duplicates a gate the function already carries.
  Sinking the import into the function lets it inherit that function's
  `#[cfg]` and drops the separate module-level one, so the reader no
  longer has to match a module-header `#[cfg(unix)]` to a function's
  `#[cfg(unix)]` by eye to confirm they agree.

- **A narrower conditional import is harder to strand.** When a
  function that used the item is deleted or its body rewritten, a
  function-local conditional import disappears with it; a
  module-level one lingers until someone notices `unused_imports`
  firing only under the matching feature set (and CI may not build
  that set). Keeping the import next to its use removes that
  feature-combination-dependent dead-import window.

The unconditional case points the other way — a plain `use` shared
or shareable across the module belongs at module scope, so that it is
written once and not duplicated. That direction is
[`underscoped-unconditional-import`](./underscoped-unconditional-import.md).
The two rules never disagree on a single `use`: this one fires only
on `#[cfg(...)]`-gated imports, its dual only on un-gated ones.

## What to lint

For every `use` item declared at **module scope** (the crate root or
any `mod` body, inline or separate-file), flag it when **all** of:

1. it carries a `#[cfg(...)]` (or `#[cfg_attr(..., ...)]` that
   expands to a `cfg`) attribute; and
2. it is **not** `pub` (nor `pub(...)`) — see *What's not in scope*;
   and
3. it is a single named leaf (`use a::b::Leaf;` / `use a::b::Leaf as
   R;`), not a glob or a brace list — see *What's not in scope*; and
4. every use of the imported binding in the module is inside the body
   of a **`#[cfg(...)]`-gated function** (a free `fn`, an
   inherent/trait-impl method, or a closure within one such body) —
   no use sits in a function that lacks a `#[cfg]` gate of its own,
   and no use sits anywhere outside a function body.

When it fires, the autofix copies the `use` (with its `#[cfg]`
attribute) to the top of **each** `#[cfg]`-gated function that uses
the item and removes the module-level declaration. The fix is
`MachineApplicable` only when the set of using functions — and that
they are all gated — is determined with certainty; otherwise the lint
emits help text and leaves the source untouched (see *Implementation
notes*).

### What's *not* in scope

- **`#[cfg(...)] pub use ...;` (and `pub(crate)` / `pub(super)` …).**
  A conditional *re-export* is part of the module's public surface;
  it cannot move into a function without changing what the module
  exports. Exempt regardless of where the name is used. (This mirrors
  the way [`private-reexport-imports`](./private-reexport-imports.md)
  blesses deliberate `pub use` re-exports.)

- **Items used outside any function body.** If the imported item
  appears in a *type position* or other item-level context — a
  `type` alias, a `const` / `static` initializer or its type, a
  struct/enum field type, a function *signature* (parameter or return
  type, as opposed to the function *body*), a trait bound, an `impl`
  header, or an attribute on an item — it cannot be function-local.
  This is the "used by things outside an `fn`, such as types and
  traits" exemption, and it is exactly condition (4) failing.

- **Items also used by a function without a `#[cfg]` gate.** If any
  function that uses the item carries no `#[cfg(...)]` of its own, the
  import supports unconditionally-compiled code and stays at module
  scope. Such a function necessarily guards its own use with an inner
  `#[cfg]`, but the function itself is unconditional, so there is no
  gated function for the import to ride into. This is condition (4)
  failing.

  ```rust
  #[cfg(unix)]
  use std::os::unix::fs::PermissionsExt;

  // Not flagged: `describe` is an un-gated function, so the import has
  // no gated home to move into and stays at module scope.
  fn describe(p: &Path) -> String {
      #[cfg(unix)]
      { return format!("{:o}", fs::metadata(p).unwrap().permissions().mode()); }
      #[cfg(not(unix))]
      { String::from("n/a") }
  }
  ```

  Note this is *not* a "used in more than one function" exemption:
  several `#[cfg]`-gated functions using the item still fire, with a
  `use` copied into each (see *What to lint*). Only a use reached from
  an un-gated function is exempt.

- **Glob and brace-list imports** (`#[cfg(...)] use foo::*;`,
  `#[cfg(...)] use foo::{A, B};`). A glob has no single leaf to
  re-point and changes name resolution wholesale; a brace list mixes
  several leaves whose uses may scatter across different functions.
  Out of scope, matching the single-leaf restriction the sibling
  import rules use.

- **`#[cfg]`-gated imports already inside a function.** Those are the
  desired end state, not a violation.

## Configuration

```toml
# dylint.toml
#
# Inactive by default. Enable in `[perfectionist].enable`. The rule has
# a single direction (sink a single-use conditional import into its
# function), so there is no `style` knob.
[perfectionist::overscoped_conditional_import]
# (no configuration)
```

The rule has one correct direction and no tunable threshold, so it
ships no knobs. The one legitimate counter-pattern — a project that
deliberately keeps all conditional imports at the module header for a
uniform import block — turns the rule off globally by leaving it out
of `[perfectionist].enable`, or suppresses an individual import with
`#[allow(perfectionist::overscoped_conditional_import)]`.

## Implementation notes

These notes fix the *shape* of the rule and deliberately stop short
of naming specific `rustc` / `clippy_utils` APIs; settle those
against the compiler during implementation.

- **A source-layout rule: re-parse in a late pass.** The trigger
  reads the *written* `#[cfg(...)]` attribute on a `use`, and that
  attribute is **gone from the HIR** for any import whose `cfg`
  evaluated true — active `#[cfg]` attributes are stripped during
  macro expansion. A plain HIR pass therefore cannot tell a
  conditional import from an unconditional one, and a pre-expansion
  `EarlyLintPass` reaches the crate root and inline `mod` blocks but
  leaves every separate-file `mod foo;` as `ModKind::Unloaded`. This
  is exactly the trap documented in
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#reaching-every-module-source-layout-rules):
  run as a **`LateLintPass`** and re-parse the crate's module files
  through `src/module_reparse.rs`, which preserves `#[cfg]` gates and
  reaches every file. Guard descent into inline `mod { ... }` with
  `live_module_spans` so a `#[cfg]`-disabled inline module is not
  linted as if it were compiled. Anchor each parked violation at its
  enclosing HIR node via `enclosing_hir::find_enclosing_hir_ids`,
  emitting through `span_lint_hir_and_then` on the `use`'s own span,
  so a per-item `#[allow]` / `#[expect]` resolves.

- **Usage analysis is a conservative syntactic scan, and that is
  sound here.** Narrowing an import's scope can never *introduce* a
  name collision — it only removes a module-scope binding — so the
  rule does not need full name resolution to be safe; the worst a
  miss can do is fail to fire. Over the re-parsed module AST, locate
  every occurrence of the imported leaf identifier (and the `as`
  alias, if any) and require that they all fall within the bodies of
  `#[cfg(...)]`-gated functions, with none in an un-gated function or
  outside any function body. The re-parse preserves the `#[cfg]`
  attributes on both the import and each candidate function, so the
  gated-vs-un-gated classification is a direct AST check. Treat
  anything that defeats a textual scan — the name
  appearing inside a macro invocation, behind another `use` of the
  same leaf, or shadowed by a local binding of the same identifier —
  as a reason **not** to fire (when unsure, don't flag), and
  downgrade the autofix from `MachineApplicable` to advisory in any
  case where the set of using functions — or that they are all gated —
  is not certain. A wrong
  *move* (as opposed to a wrong *warning*) can break compilation, so
  bias hard toward silence.

- **Parser style.** No string grammar of its own; the only parsing is
  walking the re-parsed AST. If a future knob takes path strings,
  parse them with the parser-combinator `take_*` style and the
  leading-`::` convention per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md).

- **Proc-macro suppression.** A derive can synthesize a `use` that
  carries a user-source span; if this rule's diagnostic span is
  narrower than the whole `use` item, apply the standard guard and
  add a `ui/overscoped_conditional_import_proc_macro.rs` fixture per
  the "Suppressing proc-macro-synthesised violations" section of
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).
  Because the diagnostic anchors on the entire `use` item's span, the
  rule is likely a non-participant — record that reasoning at the
  span-selection site rather than omitting it silently.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions, in particular the `perfectionist::*`
  lint-name namespacing every registered lint follows.

### Difficulty

**Hard.** Not because any single step is subtle, but because the rule
needs two views of the source that no single pass provides: the
*written* `#[cfg]` attribute (only on the re-parsed AST, which has no
resolution) and reach into every submodule file (only via
`module_reparse` in a late pass). The conservative syntactic
usage-confinement scan is the part most likely to need iteration to
get the false-positive rate to zero.

## Interaction with sibling rules

- [`underscoped-unconditional-import`](./underscoped-unconditional-import.md)
  is the exact dual and the reason this rule is scoped to
  `#[cfg(...)]` imports only: the two partition every single named
  `use` by the presence of a `cfg` gate, so they can both be active
  without ever firing on the same import or fighting over a fix.
- `perfectionist::import_granularity_mismatch`
  ([`src/rules/import_granularity_mismatch.rs`](../src/rules/import_granularity_mismatch.rs))
  and `perfectionist::import_grouping_mismatch`
  ([`src/rules/import_grouping_mismatch.rs`](../src/rules/import_grouping_mismatch.rs))
  govern the granularity and blank-line grouping of imports *at a
  given scope*; this rule decides *which scope* a conditional import
  lives in. After this rule sinks an import into a function, those
  rules reflow whatever import block results.
- [`private-reexport-imports`](./private-reexport-imports.md) shares
  the "a `pub use` re-export is a deliberate decision, leave it alone"
  exemption; both rules exempt `pub use`.

## Interaction with stock lints

No Clippy or rustc lint covers this. The lint a reader might expect,
rustc's `unused_imports`, is orthogonal — the conditional import here
*is* used, just not where it is declared, so `unused_imports` is
silent (and is precisely what eventually fires, under one feature
set, if the import is later stranded — see *Why restrict this?*).
`clippy::items_after_statements` reasons about item-vs-statement
*order within a block*, not about which scope an import belongs in.
Nothing in Clippy relates an import's declaration scope to where its
item is used or to its `#[cfg]` gate. This rule fills that gap.

## Default state

Inactive by default. "Conditional imports belong at the narrowest
scope" is a deliberate, non-mainstream project stance — many projects
prefer a single uniform import block at every module header — so the
rule ships no baseline and is opted into via `[perfectionist].enable`,
matching the project-direction rules
[`path-qualification-mismatch`](./path-qualification-mismatch.md) and
[`core-instead-of-std`](./core-instead-of-std.md). Unlike those it
expresses a single fixed direction rather than a choice, so it has no
mandatory `style` value.
