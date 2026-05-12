# `perfectionist::non_exhaustive_error`

**Default level:** `allow`  
**Source:** [`src/rules/non_exhaustive_error.rs`](../src/rules/non_exhaustive_error.rs)

> error-shaped type is missing `#[non_exhaustive]`

### What it does
Flags publicly-exposed error enums that lack a `#[non_exhaustive]`
attribute. An enum is treated as an error enum when its name ends
in `Error` (configurable) or it implements `std::error::Error`.
Publicly-exposed sum-like structs (a single field whose type is
itself an enum) follow the same rule.

"Publicly-exposed" defaults to `pub` items; `pub(crate)` and the
whole-crate "every item" sweep are configurable.

### Why restrict this?
This is a stylistic preference, not a correctness issue. Adding
a variant to an error enum is one of the most common reasons to
publish a new minor version of an error-producing library, and
`#[non_exhaustive]` is the standard way to make that addition
not a SemVer break for downstream pattern matches. Applying it
up front means future variants land without a coordinated major
release across the dependents that exhaustively match on the
enum.

The opinion is opt-in: some projects deliberately use exhaustive
error enums to force downstream consumers to handle every new
variant, and binary crates have no SemVer surface to protect.
The lint therefore defaults to `Allow` — enable it per crate
with `#![warn(perfectionist::non_exhaustive_error)]` (or
`deny`) on projects that want it.

### Example
```rust,ignore
#[derive(Debug)]
pub enum RuntimeError {
    SerializationFailure,
}
```
Use instead:
```rust,ignore
#[derive(Debug)]
#[non_exhaustive]
pub enum RuntimeError {
    SerializationFailure,
}
```

## Configuration

Configure via `dylint.toml` under `["perfectionist::non_exhaustive_error"]`. Every field is optional; the per-field prose below states the default.

### `require_for` — `RequireFor` (optional)

Visibility threshold for the rule.

### `suffixes` — `[string]` (optional)

Identifier suffixes that mark a type as "an error" purely
by name, without inspecting its trait implementations.

Setting this option **replaces** the built-in default,
rather than extending it: configuring
`suffixes = ["Failure"]` matches only `*Failure` names, not
`*Error` or `*Failure`. To keep the default suffix alongside
a project-specific one, list it explicitly:
`suffixes = ["Error", "Failure"]`.

Defaults to `["Error"]`. A type that implements
`std::error::Error` is flagged regardless of suffix.

### Types

#### `RequireFor` (enum)

##### `"pub"` (Rust: `Pub`)

Require `#[non_exhaustive]` on items that are *effectively*
reachable from outside the crate (declared `pub`, re-exported
`pub`, and not buried inside a non-`pub` module). A
`pub enum FooError` inside a non-`pub` module is not flagged
because it cannot be matched on by any downstream crate.

##### `"pub_crate"` (Rust: `PubCrate`)

In addition to the `Pub` case, require `#[non_exhaustive]`
on items literally declared `pub(crate)` (i.e., restricted
to the crate root). Items declared `pub(in some::module)`
are not promoted by this mode even if their effective reach
happens to extend to the crate root.

##### `"all"` (Rust: `All`)

Require `#[non_exhaustive]` on every error-shaped item
regardless of visibility.
