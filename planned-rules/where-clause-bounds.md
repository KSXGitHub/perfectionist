# `where_clause_bounds`

**Sources:** parallel-disk-usage *Trait Bounds*; pacquet *Trait Bounds*.

## Statement

> Prefer `where` clauses over inline bounds when there are multiple
> constraints.

## What to lint

For each item that introduces generics (functions, impls, type aliases,
traits), inspect every generic parameter. If a single parameter has more
than one bound *or* the item has more than one parameter with at least one
inline bound, suggest moving every inline bound into a `where` clause.

## Threshold

Two heuristics, both configurable:

- `where_clause_bounds.max_inline_bounds_per_param` (default `1`): an
  individual generic parameter may carry up to this many inline bounds.
- `where_clause_bounds.max_total_inline_bounds` (default `2`): across all
  generic parameters of one item, the sum of inline bounds may not exceed
  this number before a `where` clause is required.

Defaults match the examples in both source documents (one inline bound on
a single param is fine; multiple bounds or multiple params should switch
to `where`).

## Examples

```rust
// Acceptable: one bound on one parameter
fn render<W: Write>(out: &mut W) { /* ... */ }

// Bad: two bounds on one parameter
fn render<W: Write + Send>(out: &mut W) { /* ... */ }

// Good
fn render<W>(out: &mut W)
where
    W: Write + Send,
{ /* ... */ }
```

## Implementation notes

- HIR: `Generics::params` and `Generics::predicates`. Inline bounds appear
  on `GenericParam::bounds`; `where` clauses appear in
  `Generics::predicates` whose `kind` is `WherePredicateKind::BoundPredicate`.
- The lint should not double-fire on items that *already* mix inline and
  `where` styles — the suggested fix is to consolidate everything into
  `where`, not to flag the existing `where` predicates.
- Auto-fix: rendered substitution is straightforward when each bound's
  span is contiguous; emit `MachineApplicable` only when no macro
  expansions overlap the bounds list.

## Severity

Warn.
