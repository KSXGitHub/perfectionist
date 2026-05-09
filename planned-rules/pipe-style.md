# `pipe_style`

**Sources:** parallel-disk-usage *Using `pipe-trait`* and pacquet
*Using `pipe-trait`*. Both source documents prescribe the same
two-direction policy: pipe is wrong at the entry point of an
expression, and pipe is required when wrapping a method or function
chain.

## Statement

The `pipe_trait::Pipe` trait is for *continuing* an expression, not
for *starting* one. The lint enforces this from both sides:

- **Don't pipe at the start.** `value.pipe(f)` standing alone is
  noise — write `f(value)`.
- **Don't wrap a chain in a call.** `f(chain)` where `chain` itself
  contains at least one method or function call is harder to read
  than `chain.pipe(f)` — rewrite the call site as a continued pipe.

The two checks are duals of the same policy: pipe operates between
two segments of a chain, never at the boundaries. Either check fires
on a violation, both autofix to the form the policy prefers.

## Sub-checks

### `pipe_style::entry_point` (forbid pipe at chain start)

Forbid `value.pipe(f)` when `value` is not itself a method or pipe
call. This includes:

- `value.pipe(foo)` where `value` is a local binding, literal, or
  field access. Suggest `foo(value)`.
- `value.pipe(Some)` / `value.pipe(Ok)` / `value.pipe(MyVariant)`
  with the same shape. Suggest `Some(value)` / `Ok(value)` /
  `MyVariant(value)`.

The check does *not* fire when the receiver is itself a method
call, allowing the canonical continuation pattern:

```rust
report.summarize().pipe(Some)               // good
entry.file_name().pipe(OsStringDisplay::from).pipe(Some)   // good
```

### `pipe_style::wrap_chain` (require pipe over wrap-call)

Forbid `f(arg)` when `f` is unary AND `arg` is itself a `Call` or
`MethodCall` expression. Suggest `arg.pipe(f)`.

This catches the deeply-nested-call pattern that both source docs
explicitly call out:

```rust
// Bad
let data = serde_json::from_reader::<_, JsonData>(stdin());
let name = Some(OsStringDisplay::from(entry.file_name()));
let wrapped = Ok(items.iter().map(|x| x.id).collect::<Vec<_>>());

// Good
let data = stdin().pipe(serde_json::from_reader::<_, JsonData>);
let name = entry.file_name().pipe(OsStringDisplay::from).pipe(Some);
let wrapped = items.iter().map(|x| x.id).collect::<Vec<_>>().pipe(Ok);
```

The check does *not* fire when the inner expression is a leaf:

```rust
Some(value)        // good — value is a leaf
Ok(42)             // good — literal arg
MyType { x: 1 }    // good — not a call site at all
```

### How the two checks compose

The two checks are designed so that the *only* shapes either rejects
are the boundary cases:

| Shape                                  | `entry_point` | `wrap_chain` | Verdict |
|----------------------------------------|---------------|--------------|---------|
| `value.pipe(f)` (value is leaf)        | flag          | —            | bad     |
| `value.pipe(f)` (value is method call) | ok            | —            | good    |
| `f(value)` (value is leaf)             | —             | ok           | good    |
| `f(value.method())`                    | —             | flag         | bad     |
| `f(g(x))` (nested calls)               | —             | flag         | bad     |
| `chain.pipe(f).pipe(g).pipe(h)`        | ok            | ok           | good    |
| `h(g(f(x)))` (3-deep nested)           | —             | flag         | bad     |

The fixed point under both checks is the same: every transformation
of a value lives on the left of at least one `.pipe(...)`, with the
chain's first step being the original value itself.

## Configuration

```toml
[pipe_style]
# Each sub-check can be turned off independently. Defaults are both
# enforce.
entry_point = "forbid"   # or "allow" to permit pipe at chain start
wrap_chain  = "forbid"   # or "allow" to permit f(chain) call sites

# Recognised pipe trait paths. Defaults cover the canonical crate.
pipe_trait_paths = [
  "pipe_trait::Pipe::pipe",
  "pipe_trait::Pipe::pipe_ref",
  "pipe_trait::Pipe::pipe_mut",
  "pipe_trait::Pipe::pipe_as_ref",
  "pipe_trait::Pipe::pipe_as_mut",
  "pipe_trait::Pipe::pipe_borrow",
  "pipe_trait::Pipe::pipe_borrow_mut",
]
```

## What to lint

### `entry_point` direction

`LateLintPass::check_expr` on `ExprKind::MethodCall`. For each call
whose method-name matches a configured pipe path:

1. Confirm the receiver type implements `pipe_trait::Pipe` (the
   method-name match is unique enough in practice; type-confirmation
   is a second-line guard).
2. Inspect `receiver.kind`. The "tail of a chain" predicate is true
   iff the receiver is itself an `ExprKind::MethodCall` (any
   method, not just `pipe`).
3. If false, flag and suggest the rewrite `f(receiver)`.

### `wrap_chain` direction

`LateLintPass::check_expr` on `ExprKind::Call`. For each call:

1. Confirm the call is unary (exactly one argument).
2. Inspect the argument. The "is a chain" predicate is true iff the
   argument is `ExprKind::Call` or `ExprKind::MethodCall` (any
   nesting, including pipe calls themselves).
3. If true, flag and suggest the rewrite `arg.pipe(f)`.
4. The rewrite preserves any turbofish on `f`:
   `serde_json::from_reader::<_, JsonData>(stdin())` becomes
   `stdin().pipe(serde_json::from_reader::<_, JsonData>)`.

The rule deliberately doesn't recurse: if `g(f(x.m()))` is rewritten
to `f(x.m()).pipe(g)`, the next lint pass reaches `f(x.m())` and
rewrites it to `x.m().pipe(f).pipe(g)`. Fixed-point convergence in
two passes.

## Implementation notes

- `LateLintPass::check_expr` for both directions. Reuse the same
  helper that resolves a path to a `DefId` and matches against the
  configured `pipe_trait_paths`.
- The `entry_point` check resolves the method's `DefId` and
  confirms it is a method of `pipe_trait::Pipe`.
  `clippy_utils::is_diag_trait_item` won't help (Pipe is external);
  store the path as a configurable static.
- The `wrap_chain` check is purely syntactic (count args, classify
  argument's `ExprKind`). It does *not* need to know about the pipe
  trait at all — its rewrite *introduces* a pipe call, but the
  trigger condition is just "unary call wrapping a non-leaf".
- Special case: `pipe_as_ref(f)` rewrites in the `entry_point`
  direction need synthesising `.as_ref()` on the receiver. Offer
  the autofix only when the receiver type's `as_ref` is unambiguous;
  otherwise emit a help-only suggestion. The same caveat applies to
  `pipe_as_mut`, `pipe_ref`, `pipe_mut`, `pipe_borrow`,
  `pipe_borrow_mut`.
- The `wrap_chain` autofix is `MachineApplicable` for unary call
  sites whose function path is unambiguous; `MaybeIncorrect` when
  trait-method ambiguity could change which `pipe` impl is
  resolved.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Severity

Warn for both sub-checks.
