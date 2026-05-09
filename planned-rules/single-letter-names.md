# `single_letter_names`

**Sources:** parallel-disk-usage *Generic Parameter Naming* and
*Variable and Closure Parameter Naming*; pacquet sections of the same
names.

This is a family of related lints that all share the same definition of
"single-letter name" and "allowed exception". Implement them as one crate
module exposing four lints so they can be toggled independently.

## Allowed single-letter names

Both source documents agree on this allowlist:

- **Comparison closures:** the two parameters of a callback passed to
  `sort_by`, `sort_unstable_by`, `cmp`, `partial_cmp`, `min_by`, `max_by`,
  `binary_search_by`, etc. The conventional names are `a` and `b`.
- **Conventional names:** `n` for an unsigned-integer count, `f` for a
  `fmt::Formatter`.
- **Index variables `i`, `j`, `k`:** only inside short closures or
  index-based loops (`for i in 0..len`).
- **Trivial single-expression closures:** the body is a single field
  access, method call, or wrapper, e.g. `.pipe(|x| vec![x])`.
- **Fold accumulators:** `acc` for the accumulator and a single letter
  for the element, in trivial folds.
- **Test fixtures:** `let a`, `let b`, `let c` for interchangeable
  specimens *with identical roles*, in code under `#[cfg(test)]`.

Anything else with a single-letter binding is flagged.

## Sub-lints

### 1. `single_letter_generic`

Flag generic type parameters whose ident is one ASCII letter, except in
trait `impl` blocks whose body is below a small line threshold.

- HIR: `Generics::params` with `GenericParamKind::Type`.
- "Short trait impl" heuristic: the enclosing `Item` is `ItemKind::Impl`
  with `Some(of_trait)` and the impl body spans ≤ N source lines (default
  N = 20, configurable).

### 2. `single_letter_let_binding`

Flag `let x = ...;` where `x` is a single letter, outside `#[cfg(test)]`
items. Configurable allow-list for `n`, `acc`, fold accumulators inside
`fold(...)` callbacks.

- HIR: `LetStmt` whose `Pat` is a single-segment binding ident.
- Determine "in test code" by walking parents and looking for `#[cfg(test)]`
  or a `mod tests` ancestor.

### 3. `single_letter_function_param`

Flag function/method parameters whose ident is one ASCII letter, except
the conventional `n`, `f`, `i`, `j`, `k`.

- HIR: `FnDecl::inputs` for `ItemKind::Fn` and `ImplItemKind::Fn`.
- Skip parameters of trait impl methods whose name comes from the trait
  signature (the implementer cannot rename them anyway *if* the trait
  fixed them; the rule still applies to user-authored signatures).

### 4. `single_letter_closure_param`

Flag closure parameters whose ident is one ASCII letter, unless the
closure satisfies *all* of:

- Body is a single expression (no block, or a block with one tail
  expression and no statements), **and**
- The closure is the immediate argument of a call whose callee name is in
  the comparison-closure or fold allowlist (`sort_by`, `cmp`, `fold`, …),
  **or** the body is a "trivial wrapper" (a field access, method call, or
  one-arg call where the param is the sole receiver/arg).

- HIR: `ExprKind::Closure` with a body inspected via
  `cx.tcx.hir().body(closure.body)`.

## Examples

```rust
// Bad: let binding outside tests
let m = entry.metadata()?;

// Good
let metadata = entry.metadata()?;
```

```rust
// Bad: multi-line closure with single-letter param
.map(|t| {
    let columns = build_columns(t);
    format_row(&columns)
})

// Good
.map(|tree_row| {
    let columns = build_columns(tree_row);
    format_row(&columns)
})
```

```rust
// OK: comparison closure
list.sort_by(|a, b| a.name.cmp(&b.name));

// OK: trivial wrapper
.pipe(|x| vec![x])
```

## Implementation notes

- Provide `clippy_utils::is_in_test_function` / a custom equivalent for
  the cfg-test detection; walking up the HIR for `#[cfg(test)]`
  attributes is sufficient.
- Reuse a shared helper `is_allowed_short_name(ident, context)` across all
  four sub-lints to keep the allowlist in one place.
- Configuration: `dylint.toml` keys
  `single_letter_names.allowed_idents`,
  `single_letter_names.short_impl_max_lines`,
  `single_letter_names.comparison_methods` (so projects can extend the
  comparison-closure allowlist with their own DSL helpers).

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name prefixing (`perfectionist_*`)
  required for every registered lint.

## Severity

Warn for each sub-lint.
