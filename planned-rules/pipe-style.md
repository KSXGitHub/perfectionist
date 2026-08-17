# `pipe_style`

**Sources:** parallel-disk-usage *Using `pipe-trait`* and pacquet
*Using `pipe-trait`*. Both source documents prescribe the same
two-direction policy: pipe is wrong at the entry point of an
expression, and pipe is required when wrapping a method chain.
The third sub-check below (`borrow_wrapped_in_call`) is not
spelled out in either document; it is the borrowed form of the
second, and its shape is taken from a worked refactor of the
`arch-pkg-text` test suite, where every
`run_assertions(&mut Querier::new(SOURCE))` call site became
`SOURCE.pipe(Querier::new).pipe_mut(run_assertions)`.

## Statement

The `pipe_trait::Pipe` trait is for *continuing a method chain*,
not for starting one and not for replacing a free-function call.
The lint enforces this from every side:

- **Don't pipe at the start.** `value.pipe(f)` is bad when
  `value` is not itself a method call **and** the `.pipe(f)` is
  not followed by another method call. The pipe must either
  *continue* an existing method chain or be *continued* by one.
- **Don't wrap a method chain in a call.** `f(chain)` where
  `chain` is a method-call expression is harder to read than
  `chain.pipe(f)`. (A function-call argument like `f(g())` is
  *not* flagged — there's no chain to lift onto the left.)
- **Don't hide the chain behind a borrow.** `f(&mut expr)` and
  `f(&expr)` slip past the previous check by putting an `&`/`&mut`
  between the call and the expression that belongs on the left of
  a pipe. When `expr` can be lifted, write `expr.pipe_mut(f)` /
  `expr.pipe_ref(f)` — the borrow becomes the pipe method's
  receiver and disappears from the source.

The checks are duals of the same policy: pipe operates between
two segments of a method chain, never at the boundaries with
non-method code. The third closes the borrow-shaped hole in the
second.

## Sub-checks

### `pipe_style::pipe_at_chain_boundary` (forbid pipe at chain start)

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

**Avoid:**

```rust
// value is not a method call AND pipe is the tail
let result = value.pipe(foo);
let some = value.pipe(Some);

// stdin() is a free-function call, not a method call,
// AND .pipe(...) is the tail
let data = stdin().pipe(serde_json::from_reader::<_, JsonData>);
```

**Prefer:**

```rust
// entry-point form, no chain involved
let result = foo(value);
let some = Some(value);

// receiver is a method call, so .pipe(Some) continues it
let summary = report.summarize().pipe(Some);

// receiver continues to be a method call across pipes
let name = entry.file_name().pipe(OsStringDisplay::from).pipe(Some);

// receiver is a free function call BUT .pipe(...) is followed
// by another method call, so it sits between two method calls
let parsed = stdin()
    .pipe(serde_json::from_reader::<_, JsonData>)
    .map(post_process);

// receiver is a method call AND the callable inside .pipe(...)
// carries a turbofish — the turbofish lives between the parens of
// .pipe(...) and is preserved as-is.
let parsed = request.body().pipe(serde_json::from_reader::<_, MyData>);
```

### `pipe_style::chain_wrapped_in_call` (require pipe over wrap-call)

Forbid `f(arg)` when `f` is unary AND `arg` is itself an
`ExprKind::MethodCall`. Suggest `arg.pipe(f)`.

The check fires *only* on method-call arguments. A function-call
argument doesn't constitute a chain, and lifting it across pipe
would just produce the entry-point pattern that `pipe_at_chain_boundary`
forbids.

**Avoid:** arg is a method call (chain)

```rust
let name = Some(OsStringDisplay::from(entry.file_name()));
let wrapped = Ok(items.iter().map(|x| x.id).collect::<Vec<_>>());
let err = Err(parser.tokens().peek().cloned());
let lock = Arc::clone(
    locks.entry(file_path.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .value(),
);
```

**Prefer:**

```rust
let name = entry.file_name().pipe(OsStringDisplay::from).pipe(Some);
let wrapped = items.iter().map(|x| x.id).collect::<Vec<_>>().pipe(Ok);
let err = parser.tokens().peek().cloned().pipe(Err);
let lock = locks
    .entry(file_path.to_path_buf())
    .or_insert_with(|| Arc::new(Mutex::new(())))
    .value()
    .pipe(Arc::clone);
```

**Not flagged:**

```rust
// arg is a free function call, not a method call
let data = serde_json::from_reader::<_, JsonData>(stdin());

// arg is a leaf
let some = Some(value);
let ok = Ok(42);
```

### `pipe_style::borrow_wrapped_in_call` (require pipe over borrow-wrap)

Forbid `f(&arg)` and `f(&mut arg)` when `f` is unary AND `arg` is
liftable onto the left of a pipe. Suggest `arg.pipe_ref(f)` and
`arg.pipe_mut(f)` respectively.

`arg` is *liftable* in exactly two shapes, both chosen so the
suggested rewrite is a fixed point of `pipe_at_chain_boundary`:

1. **`arg` is an `ExprKind::MethodCall`.** The receiver of the new
   pipe call is a method call, so the pipe continues a chain.
   `f(&mut obj.method())` becomes `obj.method().pipe_mut(f)`.
2. **`arg` is a unary `ExprKind::Call`, `g(x)`.** Here the naive
   lift `g(x).pipe_mut(f)` would itself be a
   `pipe_at_chain_boundary` violation — the receiver is a function
   call, not a method call, and the pipe is the tail — so the head
   call is lifted in the same rewrite:
   `f(&mut g(x))` becomes `x.pipe(g).pipe_mut(f)`. Now `.pipe(g)`
   is continued by `.pipe_mut(f)` and `.pipe_mut(f)`'s receiver is
   a method call, so both pipes are legal.

Any other `arg` — a place expression (local, field, index,
deref), a nullary call, a call with two or more arguments, a
macro invocation — has no legal landing, so the check stays
quiet. Lifting those would only trade a
`borrow_wrapped_in_call` violation for a
`pipe_at_chain_boundary` one.

**Avoid:**

```rust
// `run_assertions` takes `&mut impl Querier`.
run_assertions(&mut ParsedDesc::parse(SOURCE).unwrap());
run_assertions(&mut ForgetfulQuerier::new(SOURCE));
run_assertions(&mut MemoQuerier::new(SOURCE));

// the `&` direction, with `f` taking `&Report`
summarise(&report.entries().collect::<Report>());
```

**Prefer:**

```rust
SOURCE
    .pipe(ParsedDesc::parse)
    .unwrap()
    .pipe_mut(run_assertions);
SOURCE
    .pipe(ForgetfulQuerier::new)
    .pipe_mut(run_assertions);
SOURCE
    .pipe(MemoQuerier::new)
    .pipe_mut(run_assertions);

report
    .entries()
    .collect::<Report>()
    .pipe_ref(summarise);
```

The first rewrite goes one step further than the check strictly
requires: its argument is a method call (`…unwrap()`), so shape 1
applies and `ParsedDesc::parse(SOURCE).unwrap().pipe_mut(run_assertions)`
already satisfies every sub-check. Lifting the head
`ParsedDesc::parse(SOURCE)` into `SOURCE.pipe(ParsedDesc::parse)`
is optional — permitted by `pipe_at_chain_boundary` because the
pipe is continued by `.unwrap()`, but not demanded by any check.
It is written that way above for parallelism with the two
sibling cases, where shape 2 *forces* the head pipe. Neither form
is flagged; don't add a check that requires the head lift on its
own, or every `parse(text).unwrap()` in ordinary code becomes a
violation.

**Not flagged:**

```rust
// operand is a place expression — `local.pipe_mut(f)` would just
// be a `pipe_at_chain_boundary` violation instead
run_assertions(&mut querier);
run_assertions(&mut self.querier);
run_assertions(&mut queriers[0]);

// head call is not unary, so there is no single receiver to
// lift onto the left of a pipe
run_assertions(&mut MemoQuerier::with_options(SOURCE, options));
run_assertions(&mut MemoQuerier::default());

// `f` is not unary
compare(&mut left_querier, &mut right_querier);
```

#### Why a third sub-check rather than a wider `chain_wrapped_in_call`

`chain_wrapped_in_call` classifies the argument's `ExprKind`, and
`&mut ParsedDesc::parse(SOURCE).unwrap()` is an `ExprKind::AddrOf`
— not a method call — so it stops there. Widening that check to
peel borrows would also have to change its suggestion (`pipe_ref`
/ `pipe_mut`, not `pipe`), its liftability predicate (unary calls
become liftable, because the borrow form has no acceptable
entry-point alternative), and its autofix applicability (borrow
coercions, see the implementation notes). That is a different
trigger with a different fix, so it gets its own name and its own
configuration knob.

### How the checks compose

| Shape                                       | `pipe_at_chain_boundary` | `chain_wrapped_in_call` | `borrow_wrapped_in_call` | Verdict |
|---------------------------------------------|---------------|--------------|------|---------|
| `value.pipe(f)` (value is leaf, tail)       | flag          | —            | —    | bad     |
| `g().pipe(f)` (value is fn call, tail)      | flag          | —            | —    | bad     |
| `g().pipe(f).method()` (followed by method) | ok            | —            | —    | good    |
| `obj.method().pipe(f)` (receiver is method) | ok            | —            | —    | good    |
| `f(value)` (arg is leaf)                    | —             | ok           | —    | good    |
| `f(g())` (arg is fn call)                   | —             | ok           | —    | good    |
| `f(obj.method())` (arg is method call)      | —             | flag         | —    | bad     |
| `chain.pipe(f).pipe(g)`                     | ok            | ok           | —    | good    |
| `g(f(obj.m()))`                             | —             | flag (inner) | —    | bad     |
| `f(&mut obj.method())`                      | —             | ok           | flag | bad     |
| `f(&mut g(x))` (`g` unary)                  | —             | ok           | flag | bad     |
| `f(&mut value)` (operand is a place)        | —             | ok           | ok   | good    |
| `f(&mut g(x, y))` (head not unary)          | —             | ok           | ok   | good    |
| `obj.method().pipe_mut(f)`                  | ok            | ok           | ok   | good    |
| `x.pipe(g).pipe_mut(f)`                     | ok            | ok           | ok   | good    |

`chain_wrapped_in_call` reads "ok" on every borrow row because
its argument is an `ExprKind::AddrOf`, not a method call — that
blind spot is what `borrow_wrapped_in_call` covers.

The fixed point under all three checks is the same: every method
chain stays on the left of any `.pipe(...)` it participates in,
and pipe never sits as the entry or terminal point of a
non-chain expression.

## Configuration

```toml
[pipe_style]
# Each sub-check can be turned off independently. All three default
# to enforce.
pipe_at_chain_boundary = "forbid"   # or "allow" to permit pipe at chain start
chain_wrapped_in_call  = "forbid"   # or "allow" to permit f(chain) call sites
borrow_wrapped_in_call = "forbid"   # or "allow" to permit f(&mut chain) call sites
```

The recognised pipe trait paths are hardcoded
(`pipe_trait::Pipe::pipe`, `::pipe_ref`, `::pipe_mut`,
`::pipe_deref`, `::pipe_deref_mut`, `::pipe_as_ref`,
`::pipe_as_mut`, `::pipe_borrow`, `::pipe_borrow_mut` — the nine
methods of `pipe-trait` 0.4). Forks of `pipe-trait` that expose
the same methods under a different module path are out of scope;
this is the canonical crate.

## What to lint

### `pipe_at_chain_boundary` direction

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

### `chain_wrapped_in_call` direction

`LateLintPass::check_expr` on `ExprKind::Call`. For each call:

1. Confirm the call is unary (exactly one argument).
2. Inspect the argument. The "is a method chain" predicate is
   true iff the argument is `ExprKind::MethodCall` (any depth,
   including pipe calls themselves).
3. If true, flag and suggest the rewrite `arg.pipe(f)`.
4. The rewrite preserves any turbofish on `f` exactly:
   `Foo::new::<T>(obj.method())` becomes
   `obj.method().pipe(Foo::new::<T>)`.

### `borrow_wrapped_in_call` direction

`LateLintPass::check_expr` on `ExprKind::Call`, alongside the
previous direction. For each call:

1. Confirm the call is unary (exactly one argument).
2. Confirm the argument is `ExprKind::AddrOf` with exactly one
   layer of borrow. Record its mutability: `Mutability::Mut`
   selects `pipe_mut`, `Mutability::Not` selects `pipe_ref`.
3. Classify the borrow's operand:
   - `ExprKind::MethodCall` → suggest `operand.pipe_mut(f)`.
   - `ExprKind::Call` with exactly one argument, `g(x)` → suggest
     `x.pipe(g).pipe_mut(f)`. The head lift is part of *this*
     suggestion, not a second diagnostic; emitting only
     `g(x).pipe_mut(f)` would hand the user a
     `pipe_at_chain_boundary` violation as the fix, and the two
     checks would then bounce the expression between them forever.
   - anything else → don't flag.
4. Check the borrow for coercions before offering an autofix (see
   the implementation notes).

The rule deliberately doesn't recurse: if `g(f(x.m()))` is
rewritten to `f(x.m()).pipe(g)`, the next lint pass reaches
`f(x.m())` and rewrites it to `x.m().pipe(f).pipe(g)`. Fixed-
point convergence in two passes.

## Implementation notes

- `LateLintPass::check_expr` for both directions. The two checks
  share a small helper that resolves a method-call's `DefId` and
  checks it against the hardcoded set of pipe paths.
- The `pipe_at_chain_boundary` check resolves the method's `DefId` and
  confirms it is a method of `pipe_trait::Pipe`. `clippy_utils::is_diag_trait_item`
  won't help (Pipe is external); store the path as a hardcoded
  static. The "is continued by a method chain" check requires
  `tcx.hir().parent_iter()` to inspect the immediate parent
  expression.
- The `chain_wrapped_in_call` check is purely syntactic (count args,
  classify the argument's `ExprKind`). It does *not* need to know
  about the pipe trait at all — its rewrite *introduces* a pipe
  call, but the trigger condition is just "unary call wrapping a
  method-call argument".
- Special case: `pipe_as_ref(f)` rewrites in the `pipe_at_chain_boundary`
  direction need synthesising `.as_ref()` on the receiver. Offer
  the autofix only when the receiver type's `as_ref` is
  unambiguous; otherwise emit a help-only suggestion. The same
  caveat applies to `pipe_as_mut`, `pipe_ref`, `pipe_mut`,
  `pipe_borrow`, `pipe_borrow_mut`.
- The `chain_wrapped_in_call` autofix is `MachineApplicable` for unary call
  sites whose function path is unambiguous; `MaybeIncorrect`
  when trait-method ambiguity could change which `pipe` impl is
  resolved.
- `borrow_wrapped_in_call` is the one direction where the naive
  rewrite can fail to compile, because an argument-position borrow
  may coerce and a `pipe_*` argument may not. `f(&mut expr)`
  desugars to a call whose parameter type only has to be reachable
  from `&mut T` by coercion, while `expr.pipe_mut(f)` requires `f:
  FnOnce(&mut T) -> R` exactly — an `fn` item is not coerced to fit.
  So compare the callee's parameter type against the operand's
  type before suggesting anything:
  - Parameter is `&mut T` / `&T` for the operand's own `T` (or a
    generic parameter instantiated to it, e.g. `&mut impl Querier`):
    `pipe_mut` / `pipe_ref`, autofix `MachineApplicable`.
  - Parameter is reached by a deref coercion (`&String` → `&str`,
    `&Vec<T>` → `&[T]`): the counterpart is `pipe_deref` /
    `pipe_deref_mut`. This crate's own
    `format!(…).pipe_deref(toml::from_str::<Config>)` in
    `src/rules/import_grouping_mismatch/config/tests.rs` is the shape
    — `pipe_ref` there would not compile.
  - Parameter is reached by an unsize coercion (`&mut ParsedDesc`
    → `&mut dyn Querier`), or by `AsRef` / `Borrow` at a call the
    lint can't resolve unambiguously: emit help-only text rather
    than a wrong autofix. `pipe_as_ref` / `pipe_as_mut` /
    `pipe_borrow` / `pipe_borrow_mut` cover some of these, but
    picking between them needs the target type spelled out.
  When the trigger fires but no method choice is certain, the
  diagnostic still stands — only the suggestion degrades.
- Both wrap-call directions *introduce* a `pipe_*` call, so their
  autofix is only applicable when `pipe_trait::Pipe` is in scope at
  the call site. All nine methods come from that one trait, so the
  check is a single in-scope test; when it fails, downgrade to
  help-only text mentioning the missing `use pipe_trait::Pipe;`
  rather than emitting a rewrite that doesn't compile. Inserting
  the import is out of scope — the import-placement rules
  (`perfectionist::import_grouping_mismatch`,
  `perfectionist::import_granularity_mismatch`) own where a new
  `use` belongs, and this rule shouldn't guess.
- The head lift of `borrow_wrapped_in_call`'s shape 2 reuses
  `chain_wrapped_in_call`'s rewriter: `g(x)` → `x.pipe(g)` is that
  rewrite with the method-call precondition dropped. Keep it as one
  helper so turbofish preservation (`Foo::new::<T>`) is implemented
  once.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Default state

Active by default. All three sub-checks
(`pipe_at_chain_boundary`, `chain_wrapped_in_call`, and
`borrow_wrapped_in_call`) run when the rule is active.
