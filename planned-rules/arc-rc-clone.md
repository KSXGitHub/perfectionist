# `arc_rc_clone`

**Source:** pacquet *Cloning `Arc` and `Rc`*.

## Statement

> Prefer `Arc::clone(&value)` and `Rc::clone(&value)` over `value.clone()`
> when `value` is an `Arc<T>` or `Rc<T>`.

The reasons given in the source: explicitness avoids an accidental
"expensive" clone if the binding's type changes from `Arc<T>` to `&T`,
and signals to readers that the operation is a cheap refcount bump.

## What to lint

For every method call expression whose method is `clone`, where the
receiver's type is `std::sync::Arc<T>` or `std::rc::Rc<T>` (or
`alloc::sync::Arc<T>` / `alloc::rc::Rc<T>` in `no_std` crates), suggest
`Arc::clone(&receiver)` / `Rc::clone(&receiver)`.

Do not fire when:

- The receiver is already `&Arc<T>` and the call is a deref-then-clone
  (this is rare but legal; the suggested fix is the same shape).
- The call is *itself* `Arc::clone(...)` or `Rc::clone(...)` — the
  desired form.
- The call uses the turbofish-typed form `Arc::<T>::clone(...)` or
  `Rc::<T>::clone(...)`. Functionally identical to the bare form, often
  written when the type cannot otherwise be inferred or when the author
  wants the type pinned at the call site for documentation. Same
  acceptance rule for `alloc::sync::Arc::<T>::clone(...)` and
  `alloc::rc::Rc::<T>::clone(...)`.
- The call uses the fully-qualified UFCS form
  `<Arc<T> as Clone>::clone(&value)` — also acceptable.

## Examples

```rust
// Bad
fn my_function(value: Arc<Vec<u8>>) {
    let value_clone = value.clone();
    spawn_with(value_clone);
}

// Good
fn my_function(value: Arc<Vec<u8>>) {
    let value_clone = Arc::clone(&value);
    spawn_with(value_clone);
}
```

## Implementation notes

- `LateLintPass::check_expr` on `ExprKind::MethodCall` where the method
  ident is `clone`.
- Use `clippy_utils::ty::is_type_diagnostic_item` with `sym::Arc` and
  `sym::Rc` (both are diagnostic items in rustc).
- For the autofix, render `Arc::clone(&{receiver_snippet})`. Pre-existing
  parentheses or trailing `?` need careful span handling — defer to
  `clippy_utils::source::snippet_with_applicability`.
- The `Arc::<T>::clone(...)` / `Rc::<T>::clone(...)` accepted forms
  appear in HIR as `ExprKind::Call` with a callee of `ExprKind::Path`
  whose final segment is `clone` and whose preceding segment carries a
  non-empty `GenericArgs::AngleBracketed`. Match the path's resolved
  `DefId` against `Arc::clone` / `Rc::clone` so re-exports are caught;
  the turbofish presence at the segment before `clone` is the only
  thing that distinguishes this form from the bare one for diagnostic
  purposes — both are accepted.
- The `<Arc<T> as Clone>::clone(...)` UFCS form appears as
  `ExprKind::Call` with callee `ExprKind::Path(QPath::TypeRelative(...))`
  resolving to `Clone::clone` with the qualifying type being `Arc` or
  `Rc`. Also accepted.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name prefixing (`perfectionist_*`)
  required for every registered lint.

## Interaction with `clippy::clone_on_ref_ptr`

This is essentially Clippy's `clone_on_ref_ptr` lint. The pacquet guide
exists because `clone_on_ref_ptr` is `pedantic` (off by default). We
provide it under our umbrella so projects that adopt `perfectionist` get
it without enabling all of `pedantic`.

If `clippy::clone_on_ref_ptr` is already enabled in the project, this
lint should detect that and downgrade itself to allow (or be configured
off via `dylint.toml`).

## Severity

Warn.
