# `pipe_style`

**Sources:** parallel-disk-usage *Using `pipe-trait`* and pacquet
*Using `pipe-trait`*. Both source documents prescribe the same
two-direction policy: pipe is wrong at the entry point of an
expression, and pipe is required when wrapping a method chain.
The last two sub-checks below (`borrow_wrapped_in_call` and
`call_at_pipe_chain_head`) are not spelled out in either
document. Their shape is taken from a worked refactor of the
`arch-pkg-text` test suite, where every
`run_assertions(&mut Querier::new(SOURCE))` call site became
`SOURCE.pipe(Querier::new).pipe_mut(run_assertions)` and every
`run_assertions(&mut ParsedDesc::parse(SOURCE).unwrap())` became
`SOURCE.pipe(ParsedDesc::parse).unwrap().pipe_mut(run_assertions)`
— the constant on the left in both, with nothing left calling
inward.

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
- **Don't head a piping chain with a call.** Once a chain pipes,
  it reads left-to-right; a unary call at its head reverses that
  for one step, so `g(x).unwrap().pipe_mut(f)` makes the reader
  start in the middle, go left for `x`, then turn around. Write
  `x.pipe(g).unwrap().pipe_mut(f)`. This only applies to chains
  that *do* pipe — a plain `g(x).unwrap()` is left alone.

The checks are facets of the same policy: pipe operates between
two segments of a method chain, never at the boundaries with
non-method code, and a chain that pipes at all pipes all the way
to its head. The third closes the borrow-shaped hole in the
second; the fourth is what makes the rewrites the other three
suggest converge on a single shape.

## Sub-checks

### `pipe_style::pipe_at_chain_boundary` (forbid pipe at chain start)

Forbid `value.pipe(f)` when **all** of these hold:

1. `value` is *not* an `ExprKind::MethodCall` (i.e., it isn't
   already a method call — it's a local binding, literal, field
   access, free function call, etc.).
2. The whole `value.pipe(f)` expression is *not* followed by
   another method call (i.e., the `.pipe(f)` is the tail of its
   expression).
3. The pipe is *removable*: the unpiped form — `f(value)` for
   `pipe`, `f(&mut value)` for `pipe_mut`, `f(&value)` for
   `pipe_ref`, and so on — is not itself a violation of
   `chain_wrapped_in_call` or `borrow_wrapped_in_call`.

Either of the first two being false makes the pipe acceptable: a
method-call receiver "continues" an existing chain, and a trailing
`.method()` makes the pipe "continued by" one.

Condition 3 is what keeps this check from arguing with the two
wrap-call checks. `g(x).pipe_mut(f)` satisfies conditions 1 and 2,
but unpiping it yields `f(&mut g(x))`, which
`borrow_wrapped_in_call` forbids — so this check stays quiet and
`call_at_pipe_chain_head` takes the chain instead, lifting the
head to `x.pipe(g).pipe_mut(f)`. Without condition 3 the two
checks would push the same expression back and forth forever.
Removability is decidable syntactically: unpiping is blocked
exactly when the receiver is liftable in the sense
`borrow_wrapped_in_call` defines, and the pipe method borrows.

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

The two borrow forms are treated identically — mutability picks
the pipe method and changes nothing else, so read every `pipe_mut`
below as `pipe_ref` when the borrow is shared.

`arg` is *liftable* in exactly two shapes:

1. **`arg` is an `ExprKind::MethodCall`.** The receiver of the new
   pipe call is a method call, so the pipe continues a chain.
   `f(&mut obj.method())` becomes `obj.method().pipe_mut(f)`, and
   `f(&obj.method())` becomes `obj.method().pipe_ref(f)`.
2. **`arg` is a unary `ExprKind::Call`, `g(x)`.**
   `f(&mut g(x))` becomes `x.pipe(g).pipe_mut(f)`: `.pipe(g)` is
   continued by `.pipe_mut(f)`, and `.pipe_mut(f)`'s receiver is
   now a method call, so both pipes sit between chain segments.
   `f(&g(x))` becomes `x.pipe(g).pipe_ref(f)` the same way.

Any other `arg` — a place expression (local, field, index,
deref), a nullary call, a call with two or more arguments, a
macro invocation — has no legal landing, so the check stays
quiet. `f(&mut g(x, y))` could only become `g(x, y).pipe_mut(f)`,
whose pipe has neither a method call to its left nor one to its
right and no liftable head to fix that, so
`pipe_at_chain_boundary` would flag it straight back to the
borrow form. Lifting those trades one violation for another.

**Avoid:**

```rust
// `run_assertions` takes `&mut impl Querier`.
run_assertions(&mut ParsedDesc::parse(SOURCE).unwrap());
run_assertions(&mut ForgetfulQuerier::new(SOURCE));
run_assertions(&mut MemoQuerier::new(SOURCE));

// the same call sites with a `&`-taking `run_assertions`
run_assertions(&ParsedDesc::parse(SOURCE).unwrap());
run_assertions(&ForgetfulQuerier::new(SOURCE));

// any other unary callee, here `f` taking `&Report`
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

SOURCE
    .pipe(ParsedDesc::parse)
    .unwrap()
    .pipe_ref(run_assertions);
SOURCE
    .pipe(ForgetfulQuerier::new)
    .pipe_ref(run_assertions);

report
    .entries()
    .collect::<Report>()
    .pipe_ref(summarise);
```

Both shapes end with the head piped, and the reason is
`call_at_pipe_chain_head` in both: the rewrite creates a chain
that pipes, and such a chain may not be headed by a unary call.
Neither intermediate is a resting place —
`ParsedDesc::parse(SOURCE).unwrap().pipe_mut(run_assertions)` and
`ForgetfulQuerier::new(SOURCE).pipe_mut(run_assertions)` are both
violations. Emit the fully lifted form as the suggestion, so one
pass gets there.

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

#### Why a separate sub-check rather than a wider `chain_wrapped_in_call`

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

### `pipe_style::call_at_pipe_chain_head` (forbid a call heading a piping chain)

Forbid `g(x).…` — a unary `ExprKind::Call` at the head (innermost
receiver) of a method-call chain — when the chain above it
contains at least one `pipe_*` call that is staying. Suggest
lifting the head: `x.pipe(g).…`.

Three conditions, all required:

1. The head is an `ExprKind::Call` with exactly one argument. A
   nullary head (`stdin()`) has no receiver to lift; an n-ary head
   (`g(x, y)`) has no *single* one. Neither is flagged.
2. Walking outward from the head through the chain — each parent
   an `ExprKind::MethodCall` whose receiver is the node below —
   reaches at least one pipe method.
3. No pipe in that chain is a `pipe_at_chain_boundary` violation.
   When one is, that check owns the diagnostic and its repair
   (dropping the pipe) removes the reason this one fired.

**Avoid:**

```rust
// chain pipes, head is a unary call
ParsedDesc::parse(SOURCE).unwrap().pipe_mut(run_assertions);
Command::new("git").arg("rev-parse").output().pipe(Ok);
```

**Prefer:**

```rust
SOURCE.pipe(ParsedDesc::parse).unwrap().pipe_mut(run_assertions);
"git".pipe(Command::new).arg("rev-parse").output().pipe(Ok);
```

**Not flagged:**

```rust
// no pipe anywhere in the chain — ordinary Rust, left alone
let error = parse(r#"letters = ["1"]"#).unwrap_err();
if attr_has_reason(args).is_some() { /* … */ }

// head is nullary: nothing to put on the left of a pipe
let parsed = stdin()
    .pipe(serde_json::from_reader::<_, JsonData>)
    .map(post_process);

// head is already a method call or a place expression
root.join("Cargo.toml").pipe(Manifest::from_path);
output
    .stdout
    .pipe(String::from_utf8)
    .expect("not UTF-8")
    .trim()
    .to_owned();
```

Two shapes escape *this* check but not the rule. Their heads
aren't liftable, so the head lift isn't available as a repair —
`pipe_at_chain_boundary` takes them instead, and its repair is to
drop the pipe:

```rust
// head is n-ary: `pipe_at_chain_boundary` flags the `.pipe_ref`,
// repaired as `inspect(&MemoQuerier::with_options(SOURCE, options))`
MemoQuerier::with_options(SOURCE, options).pipe_ref(inspect);

// the chain's only pipe is a `pipe_at_chain_boundary` violation
// too — `f(g(x))` is that check's repair, not a head lift
g(x).pipe(f);
```

Condition 2 is the whole scope of this check. Without it, every
`parse(text).unwrap()` in ordinary Rust becomes a violation, which
is not the policy — piping is what a chain opts into, and this
check only asks that a chain which opted in be consistent about
it. This crate's own sources bear that out: every `.pipe*()` chain
in `src/` and `tools/` is piped to its head, and every unpiped
call head sits in a chain with no pipe in it.

### How the checks compose

Columns, in order: `pipe_at_chain_boundary` (**B**),
`chain_wrapped_in_call` (**W**), `borrow_wrapped_in_call` (**R**),
`call_at_pipe_chain_head` (**H**).

| Shape                                       | B    | W            | R    | H    | Verdict |
|---------------------------------------------|------|--------------|------|------|---------|
| `value.pipe(f)` (value is leaf, tail)       | flag | —            | —    | —    | bad     |
| `g().pipe(f)` (nullary head, tail)          | flag | —            | —    | ok   | bad     |
| `g(x).pipe(f)` (unary head, tail)           | flag | —            | —    | ok   | bad     |
| `g().pipe(f).method()` (followed by method) | ok   | —            | —    | ok   | good    |
| `obj.method().pipe(f)` (receiver is method) | ok   | —            | —    | ok   | good    |
| `f(value)` (arg is leaf)                    | —    | ok           | —    | —    | good    |
| `f(g())` (arg is fn call)                   | —    | ok           | —    | —    | good    |
| `f(obj.method())` (arg is method call)      | —    | flag         | —    | —    | bad     |
| `chain.pipe(f).pipe(g)`                     | ok   | ok           | —    | ok   | good    |
| `g(f(obj.m()))`                             | —    | flag (inner) | —    | —    | bad     |
| `f(&mut obj.method())`                      | —    | ok           | flag | —    | bad     |
| `f(&obj.method())` (shared borrow)          | —    | ok           | flag | —    | bad     |
| `f(&mut g(x))` (`g` unary)                  | —    | ok           | flag | —    | bad     |
| `f(&mut value)` (operand is a place)        | —    | ok           | ok   | —    | good    |
| `f(&mut g(x, y))` (head not unary)          | —    | ok           | ok   | —    | good    |
| `g(x).pipe_mut(f)` (unary head, borrowing)  | ok   | ok           | ok   | flag | bad     |
| `g(x).unwrap().pipe_mut(f)`                 | ok   | ok           | ok   | flag | bad     |
| `g(x).unwrap()` (no pipe in the chain)      | ok   | ok           | ok   | ok   | good    |
| `obj.method().pipe_mut(f)`                  | ok   | ok           | ok   | ok   | good    |
| `x.pipe(g).pipe_mut(f)`                     | ok   | ok           | ok   | ok   | good    |

Two rows carry the whole design. `g(x).pipe(f)` is **B**'s, not
**H**'s, because the pipe is removable — `f(g(x))` is a legal
resting place. `g(x).pipe_mut(f)` is **H**'s, not **B**'s, because
the pipe is *not* removable: `f(&mut g(x))` is what **R** forbids,
so the only way out is to lift the head. Same syntactic shape,
different owner, decided by the pipe method's borrowing-ness.

`chain_wrapped_in_call` reads "ok" on every borrow row because
its argument is an `ExprKind::AddrOf`, not a method call — that
blind spot is what `borrow_wrapped_in_call` covers. Only one
shared-borrow row is listed because each `&mut` row has an
identical `&` twin: mutability selects `pipe_ref` over `pipe_mut`
and decides nothing else.

The fixed point under all four checks is the same: every method
chain stays on the left of any `.pipe(...)` it participates in,
pipe never sits as the entry or terminal point of a non-chain
expression, and a chain that pipes at all is piped from its head.
Worked end to end, the borrow-wrap shape converges like this:

```rust
run_assertions(&mut ParsedDesc::parse(SOURCE).unwrap());   // R fires
ParsedDesc::parse(SOURCE).unwrap().pipe_mut(run_assertions);   // H fires
SOURCE.pipe(ParsedDesc::parse).unwrap().pipe_mut(run_assertions);   // fixed point
```

`borrow_wrapped_in_call` suggests the last line directly, so one
pass suffices; `call_at_pipe_chain_head` still has to exist, or
the middle line — reachable by hand, or from a partially applied
fix — would be a resting place too, and the rule would have two
fixed points for one input.

## Configuration

```toml
[pipe_style]
# Each sub-check can be turned off independently. All four default
# to enforce.
pipe_at_chain_boundary  = "forbid"  # or "allow" to permit pipe at chain start
chain_wrapped_in_call   = "forbid"  # or "allow" to permit f(chain) call sites
borrow_wrapped_in_call  = "forbid"  # or "allow" to permit f(&chain) / f(&mut chain) call sites
call_at_pipe_chain_head = "forbid"  # or "allow" to permit g(x).…pipe(f) chain heads
```

Turning `call_at_pipe_chain_head` off while leaving
`borrow_wrapped_in_call` on leaves `pipe_at_chain_boundary`
without a partner for the non-removable pipes it declines to
flag: `g(x).pipe_mut(f)` then has no check that objects to it.
That is a deliberate consequence of the knob, not a hole to patch
— a user who allows call-headed pipe chains is saying that shape
is fine.

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
call whose method name matches one of the nine pipe variants:

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
4. If both predicates are false, test removability: the pipe is
   removable unless the method borrows (`pipe_ref`, `pipe_mut`,
   `pipe_deref`, `pipe_deref_mut`, `pipe_as_ref`, `pipe_as_mut`,
   `pipe_borrow`, `pipe_borrow_mut`) *and* the receiver is
   liftable in `borrow_wrapped_in_call`'s sense — those two
   together mean the unpiped form is a `borrow_wrapped_in_call`
   violation. If not removable, stay quiet and let
   `call_at_pipe_chain_head` take the chain.
5. Otherwise flag and suggest the rewrite `f(receiver)`, borrowing
   the receiver as the pipe method did.

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
   selects `pipe_mut`, `Mutability::Not` selects `pipe_ref`
   (subject to the coercion check in step 4, which may substitute
   `pipe_deref` / `pipe_as_ref` / their `_mut` counterparts). Call
   the result `pipe_borrowing` in the two branches below; nothing
   else in this direction reads the mutability.
3. Classify the borrow's operand:
   - `ExprKind::MethodCall` → suggest `operand.pipe_borrowing(f)`,
     with the operand's own chain head lifted if
     `call_at_pipe_chain_head` would demand it (it will, since the
     new chain pipes).
   - `ExprKind::Call` with exactly one argument, `g(x)` → suggest
     `x.pipe(g).pipe_borrowing(f)`.
   - anything else → don't flag.
4. Check the borrow for coercions before offering an autofix (see
   the implementation notes).

In both operand shapes the head lift is part of *this*
suggestion rather than a second diagnostic, so one pass reaches
the fixed point. It is not optional either way: in shape 2 the
unlifted `g(x).pipe_mut(f)` is what `pipe_at_chain_boundary`
declines to flag only because `call_at_pipe_chain_head` claims
it, and in shape 1 the unlifted
`g(x).unwrap().pipe_mut(f)` is a `call_at_pipe_chain_head`
violation outright.

### `call_at_pipe_chain_head` direction

`LateLintPass::check_expr` on `ExprKind::Call`, the third
`Call`-keyed direction. For each call:

1. Confirm the call is unary.
2. Confirm the call is a chain *head*: its parent is an
   `ExprKind::MethodCall` whose receiver is this call. (A call
   that is a chain head has no method call below it by
   construction — that's what makes it the head.)
3. Walk upward with `tcx.hir().parent_iter()` while each parent is
   an `ExprKind::MethodCall` whose receiver is the node below,
   collecting the chain's method calls. Stop at the first parent
   that isn't such a method call.
4. Require at least one collected method to resolve to a
   `pipe_trait::Pipe` method — this is the same `DefId` helper the
   other directions use.
5. Require that none of those pipe calls is a
   `pipe_at_chain_boundary` violation. In practice only the
   innermost pipe can be, since every later one has a method call
   as its receiver, so this is a single test on the first pipe in
   the chain.
6. Flag the head call and suggest `x.pipe(g)` in its place,
   preserving any turbofish on `g` exactly as
   `chain_wrapped_in_call` does.

The rule deliberately doesn't recurse: if `g(f(x.m()))` is
rewritten to `f(x.m()).pipe(g)`, the next lint pass reaches
`f(x.m())` and rewrites it to `x.m().pipe(f).pipe(g)`. Fixed-
point convergence in two passes.

## Implementation notes

- `LateLintPass::check_expr` for every direction. The checks share
  a small helper that resolves a method-call's `DefId` and checks
  it against the hardcoded set of pipe paths, plus a second helper
  that splits a chain into head and methods — `pipe_at_chain_boundary`
  walks up one step, `call_at_pipe_chain_head` walks the whole way.
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
  The `&` direction lands in the deref branch far more often than
  the `&mut` one — `&String` → `&str`, `&Vec<T>` → `&[T]`,
  `&PathBuf` → `&Path` are everywhere in ordinary code — so don't
  implement `Mutability::Not` as "emit `pipe_ref`" and leave the
  type comparison for later. It is the same comparison in both
  directions; only the frequency of each answer differs.
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
- Three checks emit the same head lift (`g(x)` → `x.pipe(g)`):
  `chain_wrapped_in_call` with its method-call precondition,
  `call_at_pipe_chain_head` with its pipe-in-the-chain
  precondition, and `borrow_wrapped_in_call` as part of a larger
  suggestion. Keep the rewriter itself in one helper so turbofish
  preservation (`Foo::new::<T>`) is implemented once.
- `call_at_pipe_chain_head` is the check most likely to want a
  `report_in_external_macro: false` companion guard: a chain head
  synthesised by a proc macro carries a user-source span, and the
  lift would rewrite code the user never wrote. Add the
  `clippy_utils::is_from_proc_macro` / `hir_in_external_macro`
  guard and a `ui/pipe_style_proc_macro.rs` fixture built around a
  piping chain, per the conventions file.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Default state

Active by default. All four sub-checks
(`pipe_at_chain_boundary`, `chain_wrapped_in_call`,
`borrow_wrapped_in_call`, and `call_at_pipe_chain_head`) run when
the rule is active.
