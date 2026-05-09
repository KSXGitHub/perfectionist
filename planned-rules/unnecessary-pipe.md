# `unnecessary_pipe`

**Sources:** parallel-disk-usage *Using `pipe-trait` › When NOT to use pipe*;
pacquet *Using `pipe-trait` › When NOT to use pipe*.

## Statement

> Pipe adds noise with no readability benefit when used as the entry point
> of an expression. Just call the function directly.

Concretely:

- `value.pipe(foo)` is bad when `value` is not the tail of a pre-existing
  method chain. Write `foo(value)` instead.
- `value.pipe(Some)` / `value.pipe(Ok)` / `value.pipe(MyVariant)` are bad
  for the same reason. Write `Some(value)` etc.

The rule does not fire when the pipe continues an existing chain:
`report.summarize().pipe(Some)` is good.

## What to lint

For every method call whose method name is `pipe`, `pipe_ref`,
`pipe_mut`, `pipe_as_ref`, `pipe_as_mut`, `pipe_borrow`, or
`pipe_borrow_mut` and whose receiver type implements `pipe_trait::Pipe`,
inspect the receiver:

- If the receiver is itself a method call (`MethodCall`) or a chained
  `pipe`/`pipe_*`, allow it.
- If the receiver is an identifier path, a literal, a struct/tuple
  expression, a field access of a single base, or any non-call leaf,
  flag it.

Suggest the rewrite `f(receiver)` (taking care to handle
`pipe_as_ref(f)` → `f(receiver.as_ref())`).

## Examples

```rust
// Bad
let result = value.pipe(foo);
let some = value.pipe(Some);

// Good (entry-point form)
let result = foo(value);
let some = Some(value);

// Good (continuation of a chain)
let summary = report.summarize().pipe(Some);
let name = entry.file_name().pipe(OsStringDisplay::from).pipe(Some);
```

## Implementation notes

- `LateLintPass::check_expr` on `ExprKind::MethodCall`.
- Resolve the method's `DefId` and confirm it is a method of
  `pipe_trait::Pipe`. `clippy_utils::is_diag_trait_item` won't help (Pipe
  is external); store the path as a configurable static, defaulting to
  `["pipe_trait", "Pipe", "pipe"]` and the seven variants above.
- Inspect `receiver.kind`. The "tail of a chain" predicate is true iff
  the receiver is `ExprKind::MethodCall` (any method, not just
  `pipe`).
- The autofix for `pipe_as_ref(f)` requires synthesising `.as_ref()` on
  the receiver; offer it only when the receiver type's `as_ref` is
  unambiguous (otherwise emit a help-only suggestion).

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Configuration

- `unnecessary_pipe.pipe_trait_paths` — additional paths recognised as
  pipe (for forks of the crate).
- `unnecessary_pipe.allow_at_chain_start` — defaults to `false`. Set to
  `true` to permit pipe at the start of a chain in projects that prefer
  pipe-only style.

## Severity

Warn.
