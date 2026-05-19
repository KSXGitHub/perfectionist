# `arc_rc_clone`

**Default state:** `active`  
**Source:** pacquet *Cloning `Arc` and `Rc`*.

## Status

The core rule is implemented in
[`src/rules/arc_rc_clone.rs`](../src/rules/arc_rc_clone.rs):
method-call shape detection, the bare / turbofish / UFCS qualified
forms are accepted, autofix renders `Arc::clone(&value)` /
`Rc::clone(&value)` — and drops the leading `&` when the receiver
is already typed as `&Arc<T>` / `&Rc<T>`, so chained shapes like
`arcs.first().unwrap().clone()` rewrite to
`Arc::clone(arcs.first().unwrap())`.

Still pending:

- **`clippy::clone_on_ref_ptr` interop** — when the consumer crate
  enables Clippy's `clone_on_ref_ptr` lint, this rule should detect
  that and downgrade itself to `allow` so the two lints don't
  double-fire on the same call site. The escape hatch documented in
  the "Interaction with `clippy::clone_on_ref_ptr`" section below is
  not yet wired up. A user who needs it today can still silence the
  rule by registering `perfectionist::arc_rc_clone` at `allow` in
  their project's lint config.

## Statement

> Prefer `Arc::clone(&value)` and `Rc::clone(&value)` over `value.clone()`
> when `value` is an `Arc<T>` or `Rc<T>`.

The reasons given in the source: explicitness avoids an accidental
"expensive" clone if the binding's type changes from `Arc<T>` to `&T`,
and signals to readers that the operation is a cheap refcount bump.

## Interaction with `clippy::clone_on_ref_ptr`

This is essentially Clippy's `clone_on_ref_ptr` lint. The pacquet guide
exists because `clone_on_ref_ptr` is `pedantic` (off by default). We
provide it under our umbrella so projects that adopt `perfectionist` get
it without enabling all of `pedantic`.

If `clippy::clone_on_ref_ptr` is already enabled in the project, this
lint should detect that and downgrade itself to allow (or be configured
off via `dylint.toml`).
