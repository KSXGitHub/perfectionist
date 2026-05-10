# `derive_ordering`

**Source:** parallel-disk-usage *Derive Macro Ordering* (pacquet does
not restate the rule, but does not contradict it). The exact intra-
category order parallel-disk-usage prefers is, objectively, somewhat
arbitrary — no semantic difference comes from putting `Debug` before
`Default` versus the reverse. This lint is therefore configurable;
the project picks a style, and the parallel-disk-usage shape is
recoverable as one configuration.

## Statement

The lint orders trait names *inside* a single `#[derive(...)]` list.
It does not police how derives are partitioned across multiple
`#[derive(...)]` lines — that's a layout decision left to the author.

## Configuration

```toml
[derive_ordering]
style = "preserve"   # or "alphabetical" or "prefix_then_alphabetical"

# Used only for `prefix_then_alphabetical`. Traits listed here go
# first, in this order. Traits not listed are sorted alphabetically
# after.
prefix = [
  "Debug", "Default", "Clone", "Copy",
  "PartialEq", "Eq", "PartialOrd", "Ord",
  "Hash",
]
```

Trait matching is by the final ident only. Paths like
`serde::Deserialize` are matched as `Deserialize`. A project that
wants stricter path-aware matching can re-export the relevant traits
under a single canonical name and configure on that.

## Available styles

- **`preserve`** (default) — no-op.
- **`alphabetical`** — every trait name in the list must be in
  ASCII-case-insensitive alphabetical order.
- **`prefix_then_alphabetical`** — the configured `prefix` list goes
  first in the listed order; anything else is sorted alphabetically
  after.

## Examples

Under `style = "alphabetical"`:

```rust
// Bad
#[derive(Debug, Clone, Copy)]

// Good
#[derive(Clone, Copy, Debug)]
```

Under `style = "prefix_then_alphabetical"` with the default `prefix`
above:

```rust
// Bad: PartialEq listed before Clone
#[derive(Debug, PartialEq, Clone)]

// Good: prefix order honored, Display alphabetised after the prefix
#[derive(Debug, Clone, PartialEq, Display)]
```

To recover parallel-disk-usage's "arbitrary" order verbatim, set the
prefix to its full enumeration of standard / comparison / Hash /
derive_more traits in the order parallel-disk-usage prefers.

## Implementation notes

- `EarlyLintPass::check_item` to read attributes pre-expansion, since
  `#[derive]` is consumed before HIR.
- Parse the comma-separated derive list and compare against the
  configured order.
- Trait matching uses the final path segment. Use a small helper
  `name_of(meta)` to extract the trait ident from each derive entry.
- Suggested fix is a string substitution rewriting the entire
  `#[derive(...)]` attribute. `Applicability::MachineApplicable`.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Interaction with other formatting

`cargo fmt` does *not* reorder derives. This lint is the only
mechanism for enforcing one.

## Severity

Warn. Default `style = "preserve"` keeps the rule a no-op; a project
opts in by setting a non-`preserve` style.
