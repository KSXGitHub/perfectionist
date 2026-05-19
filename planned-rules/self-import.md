# `self_import`

**Default state:** `active`  
**Source:** project convention. Companion lint to
[`import-granularity`](./import-granularity.md), which intentionally
defers all `self`-import decisions here.

## Statement

A project picks one style for handling `self` in `use` statements and
enforces it consistently. The three styles supported by this lint are:

- **`forbid`** — every form that imports a module via `self` is bad.
  Prefer the simpler bare form.
  - `use foo::bar::{self};` → `use foo::bar;`
  - `use foo::bar::self;` → `use foo::bar;` (the `::self` form is
    sometimes emitted by tools and is invalid in some positions; flag
    it as well).
  - `use foo::bar::{self, Baz};` → split into
    `use foo::bar;` and `use foo::bar::Baz;`.
- **`combined`** — adjacent `use foo::bar; use foo::bar::Baz;` should
  fold into a single `use foo::bar::{self, Baz};`.
- **`preserve`** (default) — no-op. Existing forms are accepted as
  written.

The default is `preserve` so the lint is zero-friction to adopt; a
project that has a preference enables `forbid` or `combined`
explicitly.

## Configuration

```toml
# dylint.toml
[self_import]
style = "preserve"   # or "forbid" or "combined"
```

## Style: `forbid`

Forbid every variant that uses `self` to refer to a module:

```rust
// Bad
use foo::bar::{self};
use foo::bar::self;
use foo::bar::{self, Baz};

// Good
use foo::bar;
use foo::bar::Baz;
```

The autofix replaces each `{self}` (or `::self`) with the bare module
import, and splits `{self, Baz}` into two `use` statements.

### Caveat on namespace lookup

`use foo::bar;` and `use foo::bar::{self};` are *almost* equivalent but
differ in one corner case: the bare form imports every namespace named
`bar` (type, value, macro), while the `{self}` form imports only the
module. In practice this only matters when there is a value or macro
also named `bar` in the same parent, which is rare. The `forbid`-style
autofix is therefore emitted as `Applicability::MaybeIncorrect` rather
than `MachineApplicable`; the project owner accepts the namespace-
broadening trade-off when they enable `forbid`.

## Style: `combined`

Two adjacent `use` statements where one imports a module and the
other imports an item from the same module should be folded into a
single `{self, ...}` form:

```rust
// Bad
use foo::bar;
use foo::bar::Baz;

// Good
use foo::bar::{self, Baz};
```

Adjacency is required: the lint does not reorder imports across
intervening unrelated `use` statements.

The autofix is `MachineApplicable` when the source is the
`{self}`-form (no namespace change), and `MaybeIncorrect` when the
source is a bare `use foo::bar;` (the same namespace caveat applies in
reverse — the combined form *narrows* the import to the module
namespace).

## Style: `preserve`

The lint emits nothing. Useful as a project-wide acknowledgement that
`self_import` exists but neither extreme is enforced. Default.

## What to lint

- `EarlyLintPass::check_mod`. Walk `ItemKind::Use` items in source
  order.
- For each `use` tree, recognise:
  - A leaf segment named `self` (the `::self` form).
  - A brace group containing a `self` use-tree (the `{self}` and
    `{self, X, ...}` forms).
- Under `forbid`:
  - `{self}` standalone: rewrite as the parent path without the
    braces.
  - `{self, X}`: split into two `use` statements (parent path and the
    parent-path-plus-X), preserving attributes and visibility.
  - `::self` standalone: rewrite as the parent path, with a note
    that the original form may be invalid.
- Under `combined`:
  - Walk adjacency windows of two `use` statements with matching attrs
    and visibility. If statement A imports `foo::bar` (or
    `foo::bar::{self}`) and statement B imports `foo::bar::Baz`, fold
    them into `foo::bar::{self, Baz}`.
- Under `preserve`: nothing.

## Implementation notes

- Adjacency detection mirrors the grouping logic in
  [`import-granularity`](./import-granularity.md); factor the helper
  into a shared module.
- Span construction for the autofix needs both the original `use`
  span and the trailing semicolon. `clippy_utils::source::snippet`
  helps, but `Applicability::MachineApplicable` requires no
  intervening macro expansions — fall back to `MaybeIncorrect` when
  the spans straddle one.
- The lint and `import_granularity` may both fire on the same `use`
  block if both are enabled. The order of application doesn't matter
  for correctness — apply granularity, then `self_import`, or vice
  versa, and the fixed point is the same.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Default state

The default `style = "preserve"` keeps the
pass a no-op until the project opts into `forbid` or `combined`.

## Why a separate lint from `import-granularity`

Granularity decides *how* items are grouped across `use` statements;
`self_import` decides *whether `self` is the right way to name a
module's own export*. A project might enable `module` granularity and
either `forbid` or `combined` style independently — the four
combinations are all coherent and observed in real codebases. Folding
them into one lint would force a 3-by-3 style matrix and obscure the
two distinct decisions.
