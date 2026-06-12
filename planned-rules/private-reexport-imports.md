# `private_reexport_imports`

**Source:** project convention. A churn-and-fragility hazard that AI
assistants and LSP "auto-import" actions produce constantly: faced
with `Thing` already in scope in an outer module via a non-`pub`
`use`, the easiest import they can synthesize for an inline submodule
is `use super::Thing;` — borrowing the parent's *private* import
rather than naming `Thing` where it actually lives.

## Statement

When a `use` statement names an item, the path it travels through is a
maintainability decision, and one shape is a trap: importing an item
through a **private re-export** — a binding that the named module only
holds because it itself privately `use`d the item, and which a
descendant module can reach only through ancestor privilege.

The canonical example:

```rust
// crate::outer
use crate::origin::Thing;   // private import, accidental in intent

mod inner {
    use super::Thing;       // <-- reaches `outer`'s PRIVATE re-export
    // ...
}
```

`super::Thing` here does not resolve to a definition or to a `pub`
re-export in `outer`; it resolves to `outer`'s own *private* import of
`Thing`. The submodule can name it only because a descendant inherits
access to an ancestor's private items.

The correct shape imports `Thing` from a place that owns it on its own
merits — either the module that **declares or defines** it
(`pub struct Thing`, `pub trait Thing`, …) or a module that
**re-exports it publicly** (`pub use origin::Thing;`):

```rust
mod inner {
    use crate::origin::Thing;   // names Thing where it is defined
    // ...
}
```

## Why restrict this?

This is a stylistic preference, not a correctness issue: the
private-re-export form compiles and runs identically. The objection is
to the maintenance hazard it builds in, which has three parts:

- **The intermediate import is accidental, not a deliberate API.** A
  `pub use` re-export is a considered decision to publish a name at a
  second location; a bare `use` is a local convenience. Importing
  through the latter couples a submodule to a binding that was never
  meant to be a re-export, so the submodule's source-of-truth for
  `Thing` is two hops from where `Thing` is actually defined.

- **`unused_imports` cannot retire the private import.** Once the
  outer module stops using `Thing` itself and only the inline
  submodules reach it through `super::Thing`, the outer `use` is still
  "used" — by its descendants — so rustc's `unused_imports` never
  flags it. The private import lingers indefinitely as
  invisible-but-load-bearing glue. The rule closes exactly this gap:
  the thing `unused_imports` is structurally unable to see.

- **Removing the lingering import is non-local churn.** If a mindful
  maintainer *does* notice the dead outer `use` and deletes it, every
  `use super::Thing;` in every descendant breaks at once, turning a
  one-line cleanup into a diff that touches unrelated submodules. The
  fragility is the same one the importer signed up for the moment they
  reached through ancestor privilege: the import survives only as long
  as the parent keeps a private binding it has no other reason to
  keep.

Naming `Thing` at its definition or a public re-export removes all
three: the submodule depends on a name its owner intends to expose, and
the outer module's private `use` becomes ordinary dead code that
`unused_imports` will flag and delete with no ripple.

## What to lint

For every single named import (`use P::Leaf;` or `use P::Leaf as R;`),
resolve the leaf to its target item and resolve the prefix `P` to the
module `M` the leaf is named *out of*. Flag the import when **`M`
reaches `Leaf` only through a private re-export**, i.e. all of:

1. `M` does **not** define the target item (`tcx.parent(target)` is not
   `M`); and
2. the binding for `Leaf` in `M` is an **import** (a re-export, with a
   non-empty re-export chain), not a definition; and
3. that re-export's visibility does **not** make `Leaf` nameable from
   the importing module on its own merits — the importer reaches it
   solely because it is a descendant of `M` (ancestor privilege). A
   `pub` (or otherwise importer-visible, e.g. `pub(crate)` for an
   in-crate importer) re-export is **not** flagged: that is a
   deliberate, independently-nameable re-export, which the statement
   explicitly blesses.

Equivalent intuition for (3): if the importing module were moved so it
is no longer a descendant of `M`, the import would stop compiling. That
positional fragility is the anti-pattern.

### What's *not* in scope

- **Glob imports** (`use super::*;`). There is no single leaf to
  re-point and no clean per-name rewrite; a glob that happens to pull
  in private re-exports is a separate concern.
- **`M` defines the item.** Naming a module's own public item through
  that module is the normal case.
- **Public re-exports.** `use foo::Thing;` where `foo` does
  `pub use bar::Thing;` is the blessed form — the whole point of a
  `pub use` is to offer that second name.
- **Self-referential leaves.** `use super::helper;` where `helper` is a
  *definition* in the parent (a `fn`, `struct`, inline `mod`, …) is
  fine — only a private *re-export* binding triggers the rule.

## Examples

**Avoid:** the submodule borrows the parent's private import.

```rust
use crate::origin::Thing;       // private use in `outer`

mod inner {
    use super::Thing;           // flagged
    fn f(t: Thing) { /* ... */ }
}
```

**Avoid:** same shape one level deeper, reached via `crate::`.

```rust
// crate root
use external_crate::Config;     // private use at the crate root

mod a {
    mod b {
        use crate::Config;      // flagged: `crate`'s `Config` is a
                                // private re-export, not a definition
                                // or a `pub use`
    }
}
```

**Prefer:** import from the definition / public re-export.

```rust
mod inner {
    use crate::origin::Thing;   // or `use external_crate::Config;`
}
```

**Not flagged:** the parent publishes the name deliberately.

```rust
pub use crate::origin::Thing;   // `pub use` re-export in `outer`

mod inner {
    use super::Thing;           // fine: a public re-export, nameable
                                // independent of ancestry
}
```

## Configuration

```toml
# dylint.toml
[private_reexport_imports]
# Fully-qualified import paths the lint should never flag. Use this for
# a module's *intentional* shared private import — a deliberate "local
# prelude" that descendants are meant to reach through `super::`.
allowed_paths = []
```

## Implementation notes

- A HIR `LateLintPass::check_item` on
  `ItemKind::Use(path, UseKind::Single(_))`, the same entry point as
  `perfectionist::named_prelude_imports`
  ([`src/rules/named_prelude_imports.rs`](../src/rules/named_prelude_imports.rs)).
  Resolution data (the leaf's target `DefId`, the prefix module's
  `DefId`, the re-export chain and visibility of the intermediate
  binding) only exists post-resolution, so a late pass is required.

- **This is not a source-layout rule** in the sense of
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#reaching-every-module-source-layout-rules).
  It keys off each individual `use` *item* and DefId resolution, not
  the *written order/shape* of a module body, so the HIR walk already
  reaches every compiled module — including separate-file `mod foo;`
  submodules — without the `src/module_reparse.rs` machinery. Do not
  reach for a pre-expansion `EarlyLintPass` module walk here; the same
  reasoning that keeps `named_prelude_imports` on a plain HIR
  `check_item` applies. (cfg-disabled `use`s are not the rule's
  concern — an import that is not compiled cannot be reaching a private
  re-export.)

- **Detecting the private re-export.** Resolve the prefix `P` to the
  module `M`. Find the child of `M` named `Leaf`
  (`tcx.module_children_local(M)` for a local module's
  `ModChild`s). Classify it:
  - direct definition with `tcx.parent == M` → not flagged;
  - import (`ModChild` with a non-empty `reexport_chain`) whose `vis`
    makes `Leaf` nameable from the importing module without ancestor
    privilege → not flagged (public re-export);
  - import whose `vis` does *not* reach the importer on its own → flag.

  The visibility judgment is the wedge, exactly analogous to the
  same-name-collision check in
  [`qualified-paths`](./qualified-paths.md): compare the binding's
  `ty::Visibility` against the importing module's `DefId` with
  `Visibility::is_accessible_from`, but subtract the ancestor-privilege
  that a descendant always has. Concretely, the import is flagged when
  the binding is *not* accessible from `M`'s parent scope (a sibling of
  the chain) yet *is* reachable from the importer purely positionally.
  Conservative resolution keeps the false-positive rate at zero, which
  matters because the autofix rewrites import paths.

- **Autofix.** Reuse the canonical-module resolution already built for
  `named_prelude_imports`: replace the written path span with the
  item's canonical path — the **definition** path from `tcx.def_path`
  (prefixed `crate` for the local crate, the crate name otherwise),
  preferring a nearer **`pub` re-export** module when one is publicly
  nameable. Preserve any `as` rename. Applicability is
  `MachineApplicable` when every component of the chosen path up to the
  crate root is `pub` (so it is nameable from the importer) and
  `MaybeIncorrect` otherwise — identical grading to
  `named_prelude_imports`. When all three import rules are active, the
  raw rewritten `use` line is reflowed by
  `perfectionist::import_granularity` and
  `perfectionist::import_grouping` on a subsequent `--fix` pass, so the
  suggestion need only emit one `use` per leaf.

- **Proc-macro suppression.** A `use` synthesized by a proc-macro can
  carry a user-source span and slip past `report_in_external_macro:
  false`; gate the diagnostic with `crate::common::hir_in_external_macro`
  and ship a `ui/private_reexport_imports_proc_macro.rs` regression
  fixture per the "Suppressing proc-macro-synthesised violations"
  section of
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).
  The fixture is only real if it fails with the guard removed.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

### Difficulty

**Medium.** The trigger — single named import, prefix-module
resolution, and the `ModChild` re-export-chain classification — is
mechanical and reuses `named_prelude_imports`' canonical-path autofix
wholesale. The wedge is the visibility judgment in step (3): "reachable
only by ancestor privilege" must be computed without false positives,
since the fix re-points an import. Resolve it conservatively (when in
doubt, do not flag), the same discipline `qualified_paths` applies to
its collision check.

## Interaction with sibling rules

- [`named-prelude-imports`](./named-prelude-imports.md) is the closest
  relative and the source of the shared machinery: both say "import the
  item from where it actually lives," and both carry a canonical-module
  autofix resolved from the item's `DefId`. The two never overlap on a
  single import — `named_prelude_imports` fires on a `prelude` segment
  in the *written* path, this rule on a private *re-export binding* the
  path resolves through, and a prelude is a `pub`-glob module, not a
  private import. Implement this rule's autofix by factoring the
  canonical-path resolution out of `named_prelude_imports` into a
  shared crate-internal helper rather than duplicating it.
- [`qualified-paths`](./qualified-paths.md) governs the orthogonal axis
  of *whether* to import at all (path vs. `use`); this rule governs
  *which source* an import names. They can both be active without
  conflict.
- `perfectionist::import_granularity`
  ([`src/rules/import_granularity.rs`](../src/rules/import_granularity.rs))
  and `perfectionist::import_grouping`
  ([`src/rules/import_grouping.rs`](../src/rules/import_grouping.rs))
  reflow the rewritten `use` line after this rule re-points it.

## Interaction with stock lints

No Clippy or rustc lint covers this. `unused_imports` is the lint a
reader expects to catch the lingering private import, and it
structurally cannot: the import is still used by descendants through
`super::`, so it is never "unused" (see *Why restrict this?*). Nothing
in Clippy inspects whether an import resolves through a private
re-export — `clippy::wildcard_imports`, `clippy::pub_use`,
`unreachable_pub`, and `redundant_imports` all reason about an import's
*own* form or visibility, not the binding it traverses. This rule
fills that gap.

## Default state

Active by default. The correct direction is unambiguous (import from the
definition or a public re-export), matching
[`named-prelude-imports`](./named-prelude-imports.md). The one
legitimate counter-pattern — a module that *intentionally* holds a
shared private import for its descendants to reach through `super::` —
is exempted per import via `allowed_paths`.
