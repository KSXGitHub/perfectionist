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
- The call is *itself* `Arc::clone(...)` / `Rc::clone(...)` — that's the
  desired form.
- The call sits inside a context where `clone()` has been disambiguated
  via UFCS (`<Arc<T> as Clone>::clone(&value)`) — this is also acceptable.

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
