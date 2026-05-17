# `module_item_order`

**Sources:** parallel-disk-usage *Module Organization*; pacquet *Module
Organization*.

## Statement

Within a module file, items appear in this order:

1. `pub mod` declarations.
2. `pub use` re-exports.
3. Private imports (`use ...`) and other private items.

## What to lint

In a single module body, flag any `pub mod` that appears after a `pub use`,
any `pub use` that appears after a non-`pub` `use`, and any `pub mod` that
appears after a non-`pub mod` non-import item (a struct, fn, etc.).

The rule applies to the *top-level* sequence of a module body only. Items
nested deeper (e.g., inside an `impl`) are out of scope.

## Examples

```rust
// Good
pub mod parser;
pub mod printer;

pub use parser::Parser;
pub use printer::Printer;

use std::collections::HashMap;

fn helper() { /* ... */ }
```

```rust
// Bad: pub use precedes pub mod
pub use parser::Parser;
pub mod parser;
```

```rust
// Bad: private fn precedes pub use
fn helper() {}
pub use parser::Parser;
```

## Implementation notes

- `EarlyLintPass::check_mod` (or `check_crate` for the crate root) iterates
  `Mod::items` in source order.
- Classify each item into one of `{PubMod, PubUse, PrivateUse, Other}` by
  inspecting `ItemKind` and `Visibility`.
- Track the highest category seen so far; emit a diagnostic when a later
  item belongs to a strictly earlier category.
- `clippy_utils::source::snippet_opt` can render the offending item's first
  line in the help text.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Edge cases

- `#[macro_use] extern crate foo;` predates 2018-edition imports and should
  be tolerated in the import section.
- `cfg`-gated blocks of imports (e.g., a trailing `#[cfg(unix)] use ...`
  block) are explicitly permitted by both source documents and must not
  trigger the lint when they sit *after* the main import block.

## Default state

Active by default.
