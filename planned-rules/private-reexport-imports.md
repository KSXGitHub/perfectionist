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

1. `M` does **not** define the target item; and
2. the binding for `Leaf` in `M` is an **import** (a re-export), not a
   definition; and
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

None. The rule has one correct direction, so there is no knob to tune.

The one legitimate exception — a module that *intentionally* keeps a
shared private import for its descendants to reach through `super::` —
is suppressed at the offending import with
`#[allow(perfectionist::private_reexport_imports)]` (or `#[expect(…)]`).
A per-site attribute is the right tool *because the violation is
positional*: the realistic trigger is a relative `use super::Thing;`,
whose written path is meaningful only inside its own module. There is
no stable, project-wide path string a config list could match it by —
`"super::Thing"` would exempt every `super::Thing` in the crate
regardless of which item it resolves to, and the item's canonical path
(`crate::origin::Thing`) is the path the rule wants you to *use*, not a
sensible exemption key. The attribute names the exact import; a path
list cannot. This is the deliberate reason the rule ships no
`allowed_paths`-style knob.

## Implementation notes

These notes fix the *shape* of the rule. They deliberately stop short
of naming specific `rustc` / `clippy_utils` APIs: the exact queries for
"how does module `M` bind this name?" and "is this binding visible
here?" should be settled against the compiler during implementation,
not trusted from this file. Treat the points below as the design, not
as verified API.

- **A late pass over single named imports.** The rule triggers on each
  `use P::Leaf;` / `use P::Leaf as R;`. What it must know — what `Leaf`
  ultimately resolves to, and *how* the module `P` binds `Leaf` (its own
  definition, a public re-export, or a private import) — exists only
  after name resolution, so this is a `LateLintPass`, not a
  pre-expansion `EarlyLintPass`. `named_prelude_imports`
  ([`src/rules/named_prelude_imports.rs`](../src/rules/named_prelude_imports.rs))
  is the existing rule with the nearest shape; follow its structure.

- **Not a source-layout rule.** The rule keys off individual `use`
  items, not the written order/shape of a module body, so a plain HIR
  pass already reaches every compiled module — including separate-file
  `mod foo;` submodules — the way `named_prelude_imports` does. It does
  *not* need `src/module_reparse.rs`. See
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#reaching-every-module-source-layout-rules)
  for why that helper exists and why it is unnecessary here.

- **The classification to implement.** For the prefix module `P` and
  the leaf name, decide which of three cases holds: `P` *defines* the
  item (fine); `P` *re-exports* it with a visibility that lets the
  importer name it on its own merits (fine); or `P` only reaches it
  through a *private* import the importer can see purely because it is a
  descendant of `P` (flag). The third case is the rule. The
  visibility comparison is the part most likely to be subtle — resolve
  it conservatively (when unsure, do not flag), since the autofix
  rewrites import paths and a false positive misdirects an import.

- **Autofix.** Re-point the import at where the item is actually owned —
  its definition or a public re-export — reusing the canonical-path
  resolution already built for `named_prelude_imports` rather than
  duplicating it. Preserve any `as` rename, and grade applicability the
  way that rule does (machine-applicable only when the rewritten path is
  publicly nameable from the importer). When the import rules are all
  active, `perfectionist::import_granularity_mismatch` and
  `perfectionist::import_grouping_mismatch` reflow the rewritten line on a later
  `--fix` pass, so the suggestion need only emit one `use` per leaf.

- **Proc-macro suppression.** If a proc-macro can synthesize a flagged
  `use` carrying a user-source span, gate the diagnostic with the
  standard guard and add a regression fixture, per the "Suppressing
  proc-macro-synthesised violations" section of
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations).

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions, in particular the `perfectionist::*`
  lint-name namespacing every registered lint follows.

### Difficulty

**Medium.** Triggering on single named imports and reusing
`named_prelude_imports`' canonical-path autofix is mechanical. The wedge
is the visibility classification — telling a private import reached by
ancestor privilege apart from a genuine public re-export — which must be
conservative to keep false positives at zero, since the fix re-points an
import.

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
- [`path-qualification-mismatch`](./path-qualification-mismatch.md) governs the orthogonal axis
  of *whether* to import at all (path vs. `use`); this rule governs
  *which source* an import names. They can both be active without
  conflict.
- `perfectionist::import_granularity_mismatch`
  ([`src/rules/import_granularity_mismatch.rs`](../src/rules/import_granularity_mismatch.rs))
  and `perfectionist::import_grouping_mismatch`
  ([`src/rules/import_grouping_mismatch.rs`](../src/rules/import_grouping_mismatch.rs))
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
is suppressed per import with
`#[allow(perfectionist::private_reexport_imports)]`, not a config knob
(see *Configuration*).
