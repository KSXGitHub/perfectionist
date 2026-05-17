# `pipe_style`

**Sources:** parallel-disk-usage *Using `pipe-trait`* and pacquet
*Using `pipe-trait`*. Both source documents prescribe the same
two-direction policy: pipe is wrong at the entry point of an
expression, and pipe is required when wrapping a method chain.

## Statement

The `pipe_trait::Pipe` trait is for *continuing a method chain*,
not for starting one and not for replacing a free-function call.
The lint enforces this from both sides:

- **Don't pipe at the start.** `value.pipe(f)` is bad when
  `value` is not itself a method call **and** the `.pipe(f)` is
  not followed by another method call. The pipe must either
  *continue* an existing method chain or be *continued* by one.
- **Don't wrap a method chain in a call.** `f(chain)` where
  `chain` is a method-call expression is harder to read than
  `chain.pipe(f)`. (A function-call argument like `f(g())` is
  *not* flagged — there's no chain to lift onto the left.)

The two checks are duals of the same policy: pipe operates between
two segments of a method chain, never at the boundaries with
non-method code.

## Sub-checks

### `pipe_style::entry_point` (forbid pipe at chain start)

Forbid `value.pipe(f)` when **both** of these hold:

1. `value` is *not* an `ExprKind::MethodCall` (i.e., it isn't
   already a method call — it's a local binding, literal, field
   access, free function call, etc.).
2. The whole `value.pipe(f)` expression is *not* followed by
   another method call (i.e., the `.pipe(f)` is the tail of its
   expression).

Either condition being true makes the pipe acceptable: a method-
call receiver "continues" an existing chain, and a trailing
`.method()` makes the pipe "continued by" one.

```rust
// Bad: value is not a method call AND pipe is the tail
let result = value.pipe(foo);
let some = value.pipe(Some);

// Bad: stdin() is a free-function call, not a method call,
// AND .pipe(...) is the tail
let data = stdin().pipe(serde_json::from_reader::<_, JsonData>);

// Good (entry-point form, no chain involved)
let result = foo(value);
let some = Some(value);

// Good: receiver is a method call, so .pipe(Some) continues it
let summary = report.summarize().pipe(Some);

// Good: receiver continues to be a method call across pipes
let name = entry.file_name().pipe(OsStringDisplay::from).pipe(Some);

// Good: receiver is a free function call BUT .pipe(...) is followed
// by another method call, so it sits between two method calls
let parsed = stdin()
    .pipe(serde_json::from_reader::<_, JsonData>)
    .map(post_process);

// Good: receiver is a method call AND the callable inside .pipe(...)
// carries a turbofish — the turbofish lives between the parens of
// .pipe(...) and is preserved as-is.
let parsed = request.body().pipe(serde_json::from_reader::<_, MyData>);
```

### `pipe_style::wrap_chain` (require pipe over wrap-call)

Forbid `f(arg)` when `f` is unary AND `arg` is itself an
`ExprKind::MethodCall`. Suggest `arg.pipe(f)`.

The check fires *only* on method-call arguments. A function-call
argument doesn't constitute a chain, and lifting it across pipe
would just produce the entry-point pattern that `entry_point`
forbids.

```rust
// Bad: arg is a method call (chain)
let name = Some(OsStringDisplay::from(entry.file_name()));
let wrapped = Ok(items.iter().map(|x| x.id).collect::<Vec<_>>());
let err = Err(parser.tokens().peek().cloned());

// Good
let name = entry.file_name().pipe(OsStringDisplay::from).pipe(Some);
let wrapped = items.iter().map(|x| x.id).collect::<Vec<_>>().pipe(Ok);
let err = parser.tokens().peek().cloned().pipe(Err);

// Not flagged: arg is a free function call, not a method call
let data = serde_json::from_reader::<_, JsonData>(stdin());

// Not flagged: arg is a leaf
let some = Some(value);
let ok = Ok(42);
```

### How the two checks compose

| Shape                                       | `entry_point` | `wrap_chain` | Verdict |
|---------------------------------------------|---------------|--------------|---------|
| `value.pipe(f)` (value is leaf, tail)       | flag          | —            | bad     |
| `g().pipe(f)` (value is fn call, tail)      | flag          | —            | bad     |
| `g().pipe(f).method()` (followed by method) | ok            | —            | good    |
| `obj.method().pipe(f)` (receiver is method) | ok            | —            | good    |
| `f(value)` (arg is leaf)                    | —             | ok           | good    |
| `f(g())` (arg is fn call)                   | —             | ok           | good    |
| `f(obj.method())` (arg is method call)      | —             | flag         | bad     |
| `chain.pipe(f).pipe(g)`                     | ok            | ok           | good    |
| `g(f(obj.m()))`                             | —             | flag (inner) | bad     |

The fixed point under both checks is the same: every method
chain stays on the left of any `.pipe(...)` it participates in,
and pipe never sits as the entry or terminal point of a
non-chain expression.

## Configuration

```toml
[pipe_style]
# Each sub-check can be turned off independently. Defaults are both
# enforce.
entry_point = "forbid"   # or "allow" to permit pipe at chain start
wrap_chain  = "forbid"   # or "allow" to permit f(chain) call sites
```

The recognised pipe trait paths are hardcoded
(`pipe_trait::Pipe::pipe`, `::pipe_ref`, `::pipe_mut`,
`::pipe_as_ref`, `::pipe_as_mut`, `::pipe_borrow`,
`::pipe_borrow_mut`). Forks of `pipe-trait` that expose the same
methods under a different module path are out of scope; this is
the canonical crate.

## What to lint

### `entry_point` direction

`LateLintPass::check_expr` on `ExprKind::MethodCall`. For each
call whose method name matches one of the seven pipe variants:

1. Confirm the receiver type implements `pipe_trait::Pipe` (the
   method-name match is unique enough in practice; type-
   confirmation is a second-line guard).
2. Inspect `receiver.kind`. The "continues a method chain"
   predicate is true iff the receiver is itself an
   `ExprKind::MethodCall`.
3. If false, walk *upward* from the pipe call. The "is continued
   by a method chain" predicate is true iff the immediate parent
   expression is an `ExprKind::MethodCall` whose receiver is this
   pipe call.
4. If both predicates are false, flag and suggest the rewrite
   `f(receiver)`.

### `wrap_chain` direction

`LateLintPass::check_expr` on `ExprKind::Call`. For each call:

1. Confirm the call is unary (exactly one argument).
2. Inspect the argument. The "is a method chain" predicate is
   true iff the argument is `ExprKind::MethodCall` (any depth,
   including pipe calls themselves).
3. If true, flag and suggest the rewrite `arg.pipe(f)`.
4. The rewrite preserves any turbofish on `f` exactly:
   `Foo::new::<T>(obj.method())` becomes
   `obj.method().pipe(Foo::new::<T>)`.

The rule deliberately doesn't recurse: if `g(f(x.m()))` is
rewritten to `f(x.m()).pipe(g)`, the next lint pass reaches
`f(x.m())` and rewrites it to `x.m().pipe(f).pipe(g)`. Fixed-
point convergence in two passes.

## Implementation notes

- `LateLintPass::check_expr` for both directions. The two checks
  share a small helper that resolves a method-call's `DefId` and
  checks it against the hardcoded set of pipe paths.
- The `entry_point` check resolves the method's `DefId` and
  confirms it is a method of `pipe_trait::Pipe`. `clippy_utils::is_diag_trait_item`
  won't help (Pipe is external); store the path as a hardcoded
  static. The "is continued by a method chain" check requires
  `tcx.hir().parent_iter()` to inspect the immediate parent
  expression.
- The `wrap_chain` check is purely syntactic (count args,
  classify the argument's `ExprKind`). It does *not* need to know
  about the pipe trait at all — its rewrite *introduces* a pipe
  call, but the trigger condition is just "unary call wrapping a
  method-call argument".
- Special case: `pipe_as_ref(f)` rewrites in the `entry_point`
  direction need synthesising `.as_ref()` on the receiver. Offer
  the autofix only when the receiver type's `as_ref` is
  unambiguous; otherwise emit a help-only suggestion. The same
  caveat applies to `pipe_as_mut`, `pipe_ref`, `pipe_mut`,
  `pipe_borrow`, `pipe_borrow_mut`.
- The `wrap_chain` autofix is `MachineApplicable` for unary call
  sites whose function path is unambiguous; `MaybeIncorrect`
  when trait-method ambiguity could change which `pipe` impl is
  resolved.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Default state

Active by default. Both sub-checks (`leading_pipe` and
`wrapped_chain`) run when the rule is active.
