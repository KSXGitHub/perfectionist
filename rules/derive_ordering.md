# `perfectionist::derive_ordering`

**Default level:** `warn`  
**Source:** [`src/rules/derive_ordering.rs`](../src/rules/derive_ordering.rs)

> trait names in a `#[derive(...)]` list are not in the configured order

### What it does
Enforces a project-wide ordering of trait names inside a single
`#[derive(...)]` list. Three styles are configurable via
`style`:
- `preserve` (default) — no-op.
- `alphabetical` — every trait name must be in
  ASCII-case-insensitive alphabetical order.
- `prefix_then_alphabetical` — the configured `prefix` list of
  traits goes first, in the listed order; remaining traits are
  sorted alphabetically after.

Trait matching is by the final path segment, so
`serde::Deserialize` is matched as `Deserialize`. The lint
does not police how derives are partitioned across multiple
`#[derive(...)]` lines — that's a layout decision left to the
author.

### Why restrict this?
This is a stylistic preference, not a correctness issue. The
trait order inside `#[derive(...)]` has no semantic effect:
`#[derive(Debug, Clone)]` and `#[derive(Clone, Debug)]`
produce identical impls. A project-wide convention makes
derive lists scan uniformly across the codebase. `cargo fmt`
does not reorder derives, so this lint is the only mechanism
for enforcing one.

### Example
Under `style = "alphabetical"`:
```rust,ignore
#[derive(Debug, Clone, Copy)]
struct Point;
```
Use instead:
```rust,ignore
#[derive(Clone, Copy, Debug)]
struct Point;
```

## Configuration

Configure via `dylint.toml` under `["perfectionist::derive_ordering"]`. Every field is optional; the per-field prose below states the default.

### `style` — `Style` (optional)

Ordering policy. Defaults to `preserve`, which is a no-op;
a project opts in by setting `alphabetical` or
`prefix_then_alphabetical`.

### `prefix` — `[string]` (optional)

Trait names that must appear first under the
`prefix_then_alphabetical` style, in the order they should
appear. Ignored under other styles. Matched by the final
path segment, so a configured `"Debug"` matches both
`Debug` and `std::fmt::Debug` written in the source.

### Types

#### `Style` (enum)

##### `"preserve"` (Rust: `Preserve`)

No-op. The lint emits nothing.

##### `"alphabetical"` (Rust: `Alphabetical`)

Every trait name must appear in ASCII-case-insensitive
alphabetical order.

##### `"prefix_then_alphabetical"` (Rust: `PrefixThenAlphabetical`)

Traits listed in the configured `prefix` come first, in the
listed order; remaining traits are sorted alphabetically
after.
