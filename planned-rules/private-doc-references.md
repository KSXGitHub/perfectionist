# `private_doc_references`

**Source:** pacquet *Documentation comments*.

## Statement

> A doc comment of a `pub` item must not name an item more private than
> itself.

The full hierarchy: a `pub` item's `///` may not reference a `pub(crate)`
or private item; a `pub(crate)` item's `///` may not reference a private
item; etc. References can be intra-doc links, plain backticks, or even
unlinked prose mentions of an identifier — once that identifier matches
the name of a more-private item in scope, rustdoc's output contains a
broken or invisible reference.

## What to lint

For each item with `///` or `//!`:

1. Determine the item's effective visibility (pub / pub(crate) / pub(super)
   / private).
2. For every backticked identifier or intra-doc link in the doc comment,
   resolve it.
3. Compute the resolved item's visibility.
4. Flag if the referenced item is strictly more private than the
   documented item.

## Examples

```rust
// Bad: public doc references a private helper
/// Builds the lockfile by walking dependencies.
///
/// Internally calls [`walk_deps_inner`] to handle cycles.
pub fn build_lockfile() { /* ... */ }

fn walk_deps_inner() { /* ... */ }
```

```rust
// Good: public doc references public observable behavior
/// Builds the lockfile by walking dependencies.
///
/// Cycles in the dependency graph are reported as
/// [`LockfileError::CycleDetected`].
pub fn build_lockfile() { /* ... */ }
```

## Implementation notes

- `LateLintPass`. The `tcx.visibility(def_id)` query gives an item's
  effective visibility as `ty::Visibility`. Comparing two
  `ty::Visibility` values reduces to checking that the documented item's
  visibility is *not* a subset of the referenced item's restriction.
- Reuse the doc-comment parser from `intra_doc_links` so the two lints
  share a tokenisation pass.
- For unlinkable references (plain prose mentions that happen to match a
  symbol name), keep the lint conservative: require the mention to be
  inside backticks. Bare-word matching is too noisy.

## Suggested fix

The source guide gives two options:

1. Widen the visibility of the referenced item (or add a `pub` re-export).
2. Move the explanation into a regular `//` comment on the implementation.

The lint emits both as help suggestions; neither is mechanically
applicable.

## Severity

Warn.
