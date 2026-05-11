# `macro_argument_binding`

**Source:** project convention. The motivating bug:

```rust
debug_assert_eq!(my_set.insert(new_item), None, "Something went wrong! `new_item` wasn't new");
```

In debug builds this works: `insert` runs, the result is compared
against `None`, and the assertion panics if `new_item` was a
duplicate. In release builds `debug_assert_eq!` expands to nothing
at all — the arguments are *not evaluated* — so `insert` never
runs, `new_item` is silently dropped, and `my_set` ends the
function in a different state from the one the author intended.
The bug only shows up when the binary is finally built with
`--release` and behaves differently from every test run.

The fix is to bind the call to a `let` first, then pass the
binding to the macro:

```rust
let was_new = my_set.insert(new_item).is_none();
debug_assert!(was_new, "Something went wrong! `new_item` wasn't new");
```

`debug_assert*` is the most famous offender, but the trap is
general: a function-like or array-like macro can evaluate any
given argument zero times, once, or many times depending on its
matcher, and the call-site cannot tell which without reading the
macro's body. Functions guarantee exactly-once evaluation per
argument; macros do not, even when the call shape looks
identical.

## Why is this bad?

The `debug_assert*` case is an objective defect — the program's
runtime behaviour differs between debug and release builds in a
way the author did not intend, and the difference is invisible at
the call site. A duplicate insertion silently succeeds in
release; a `?` operator that would have short-circuited an
error in debug never runs in release, so the function proceeds
past the assert as if the call had returned `Ok`; an iterator's
`.next()` quietly advances in one build configuration but not
another. None of these are stylistic preferences — they're
bugs.

The general form ("any function-like or array-like macro may
evaluate an argument zero or many times") is partly a correctness
concern and partly a defensive coding stance:

- **Correctness** for macros that conditionally skip evaluation
  (`debug_assert*`, `cfg!`, custom `#[cfg(test)]` wrappers, log
  macros below the configured filter level in some
  implementations).
- **Correctness** for macros that evaluate an argument multiple
  times (anything implementing a syntactic transformation —
  `min!`/`max!`-style macros, n-arg-tuple builders, retry-loop
  macros). A side-effecting expression repeated produces wrong
  results.
- **Defensive** for unknown macros where the call-site author
  doesn't know the evaluation count and cannot find out without
  reading the matcher. A `let` binding makes the code work
  regardless.

The rule's default configuration flags only the cases where the
trap is real (known-conditional macros and macros not on the
curated safe list). The blanket-ban mode is opt-in for projects
that want the defensive stance everywhere.

## Statement

For a function-like (`name!(...)`) or array-like (`name![...]`)
macro invocation:

- If the macro is on the **denylist** of macros known to evaluate
  arguments conditionally or repeatedly, every non-trivial
  top-level argument is flagged. The set of accepted argument
  shapes is the trivial set defined under
  "What counts as a 'non-trivial' argument" below — literals,
  paths, `&path`, `path.field`, `*path`, and casts.
- If the macro is on the **allowlist** of macros known to
  evaluate each top-level argument exactly once, the argument
  shape is unconstrained.
- For every other macro, behaviour depends on the selected
  mode — see "Five eligibility modes" below. Briefly:
  `denylist_only` and `matcher_based` (for proc macros) skip
  the invocation, `blanket` flags it, and `allowlist_denylist`
  consults the configured `unknown_macro_policy`.

Curly-brace macro invocations (`name! { ... }`) are out of scope.
They are conventionally DSL bodies where the evaluation contract
is intentional (`thread_local! { static FOO: ...; }`,
`quote! { fn foo() { ... } }`, `html! { <div>{value}</div> }`),
not function-call-like argument lists.

The call-site delimiter is the *only* signal the lint uses to
classify a macro invocation. A declarative macro's body does not
fix its call shape; the same `macro_rules!` accepts `name!(...)`,
`name![...]`, and `name! { ... }` interchangeably. The lint
therefore branches on the invocation's `Delimiter`, not on
anything about the definition.

### What counts as a "non-trivial" argument

A "trivial" argument is one whose evaluation has no observable
effect and produces the same value every time. Trivial arguments
are accepted even by macros on the denylist, because zero-or-many
evaluations of a trivial expression are indistinguishable from
exactly-one.

Trivial:

- Literals (`42`, `"hello"`, `true`, `'a'`).
- Path expressions resolving to a `const`, `static`, local
  binding, function name, or unit / tuple variant
  (`MAX_VALUE`, `Foo::BAR`, `count`, `Result::Ok`).
- A reference to a trivial expression (`&count`,
  `&mut buffer`).
- A field access or tuple-index on a trivial base
  (`config.threshold`, `point.0`).
- A unary deref of a trivial expression (`*ptr`).
- A trivial expression annotated with a type (`x as u64`,
  `42_u8`).

Non-trivial (anything not in the list above), e.g.:

- Function calls (`compute()`, `Foo::new()`).
- Method calls (`map.insert(k, v)`, `iter.next()`).
- `?` expressions (`fallible()?`).
- `.await` expressions.
- Macro invocations (`other_macro!(x)`).
- Block expressions (`{ ... }`).
- Closure expressions, even if not invoked here.
- `if`, `match`, `loop`, `while`, `for`.
- Assignment / compound-assignment.
- Range expressions whose endpoints are non-trivial.
- Binary expressions whose operands are non-trivial.

The trivial / non-trivial split is purely *syntactic*. The lint
does not consult the type checker for purity (which Rust does not
expose anyway). A `const fn` call is treated as non-trivial; the
author can hoist it to a `const` if they want it accepted.

## Five eligibility modes

The user's question — what difficulty levels exist between
"forbid everything" and "fully read the macro definition" — is
answered by the configuration ladder below. The modes are
ordered by *implementation* cost, not by some clean monotone
relationship on what they flag: Mode 3 relaxes Mode 2's
denial set, and Mode 4 reshapes it. The default is mode 2
(curated allowlist + denylist).

### Mode 0 — denylist-only (the simplest possible rule)

*Easier than the user's "easiest".* Ship a hard-coded denylist of
known-conditional macros — `debug_assert!`, `debug_assert_eq!`,
`debug_assert_ne!`, `cfg!`, and a handful of similar shapes — and
flag only those. Every other macro is silently accepted.

The implementation is a name-set lookup keyed on the resolved
`DefId` plus a non-trivial-argument predicate. No allowlist, no
matcher walk, no user configuration beyond extending the
denylist. The rule's surface area is "this exact set of
core/std macros, for this exact reason".

This mode exists for projects that want to enable
`perfectionist::macro_argument_binding` without auditing their
third-party macro use — they only care about not getting bitten
by `debug_assert*`. Set `mode = "denylist_only"`.

### Mode 1 — blanket ban

The user's "easiest, dumbest" mode. Every function-like and
array-like macro invocation is flagged when given a non-trivial
top-level argument, regardless of whether the macro is known
safe. Curly-brace invocations remain out of scope per the
"Statement" section.

This mode is deliberately not the default — `format!("hello
{name}", compute())` is fine in practice and reading every macro
invocation as a footgun is exhausting. Projects that want the
maximum-paranoia stance can opt in by setting
`mode = "blanket"`. Set the `extra_allowlist` knob to whitelist
specific macros that the project trusts.

### Mode 2 — curated allowlist + denylist (default)

The user's "still easy" mode, augmented with the denylist
spelled out in mode 0. Three name-lookups decide each
invocation:

1. **Denylist hit**: flag every non-trivial argument. The
   denylist defaults to `debug_assert!`, `debug_assert_eq!`,
   `debug_assert_ne!`, `cfg!`, and any user-added entries.
2. **Allowlist hit**: accept unconditionally. The allowlist
   defaults to the same `core` / `std` and well-known
   third-party set as
   [`macro-trailing-comma`](./macro-trailing-comma.md) —
   `format!`, `println!`, `vec!`, `write!`, `assert!`,
   `assert_eq!`, the `log::*` and `tracing::*` families,
   `anyhow!`, and similar.
3. **Neither**: skip silently. Unknown macros are not flagged
   by default. A project that wants stricter behaviour
   reaches for mode 1 or mode 4.

The default rejecting only the denylist (rather than every
unlisted macro) is a usability choice. The user's framing of
"still easy" was that unlisted macros get flagged; in practice
that produces too much noise during initial rule adoption.
Projects can set `unknown_macro_policy = "deny"` to recover
the strict variant.

### Mode 3 — mode 2 with expression-side bypass

A bypass that layers on top of any of the modes above. The
bypass does not change the trivial / non-trivial split —
that classification stays as defined under "What counts as a
'non-trivial' argument". Instead, it adds an *accept rule*:
even when the macro would otherwise fire (denylisted, or
denied under mode 1's blanket), accept the invocation if
every top-level argument's outermost shape is either trivial
or a function / method call whose own sub-expressions are all
trivial. The canonical example is `Arc::clone(&x)`: a `Call`
whose sole argument `&x` is a reference to a path, both of
which are trivial. The bypass's recursion stops at any
non-trivial sub-expression.

The motivation is that `debug_assert_eq!(my_set.contains(&k),
true)` is *also* unsafe-feeling but ultimately fine — the
worst case is "contains is not called in release", which has
no observable effect because `contains` is pure. The bypass
captures the heuristic "non-mutating method calls are safe to
re-evaluate or skip" by accepting any call whose arguments are
themselves trivial (typically `&path` / `path`).

The implementation cost over mode 2 is one extra match arm
plus a recursive descent through `ExprKind::Call`,
`ExprKind::MethodCall`, and `ExprKind::Field`. The accuracy
gain is large in real codebases.

Enable with `expression_bypass = true`. Default off — the
heuristic is approximate (it cannot tell a pure method from an
impure one), and projects that want the strictest reading
should leave it off. Projects that find the default too noisy
turn it on as the first knob to adjust.

### Mode 4 — matcher-based declarative-macro analysis

The user's "hard" mode. Layered on top of mode 2: the
denylist and allowlist are consulted first, and the matcher
walk runs only on `macro_rules!` macros that *would otherwise
be unknown*. An invocation that resolves to an allowlisted
macro is still accepted unconditionally; an invocation that
resolves to a denylisted macro is still flagged. The matcher
walk turns an "unknown" verdict into a justified
allowlist-or-flag decision rather than overriding the curated
lists. For a `macro_rules!` macro reached by the walk (its
definition visible to the compiler — current crate, or a
dependency whose macro body rustc still has on hand):

1. Determine which arm of the macro matches the call.
2. For each `$name:expr` capture in that arm, count its
   occurrences in the corresponding RHS.
   - Exactly one occurrence, with the capture not nested
     inside any `$( ... )*` / `$( ... )+` / `$( ... )?`
     repetition that could change its evaluation count:
     the argument is evaluated exactly once. Eligible
     (treat the argument's macro as if it were on the
     allowlist).
   - Zero occurrences (the capture is matched but discarded):
     the argument is never evaluated. Flag with a "the
     argument is unused in this expansion" diagnostic.
   - Two or more occurrences, or any occurrence inside a
     repetition / conditional fragment: the argument may be
     evaluated more than once or skipped. Flag with the
     "this macro does not evaluate arguments exactly once"
     diagnostic.
3. If multiple arms could match the same invocation
   ambiguously, fall back to the *most conservative* answer
   across all candidate arms.

Matcher analysis does not extend to procedural macros — the
expansion is custom Rust code, not introspectable from the
matcher. Proc macros remain governed by the user's allowlist /
denylist configuration.

Set `mode = "matcher_based"` to enable. Defaults off; this
mode is the most expensive to implement (see "Why
matcher-based is harder than name-based" in
[`macro-trailing-comma`](./macro-trailing-comma.md), which
faces the same matcher-access infrastructure work) and the
matcher walker can be reused between this rule and
`macro-trailing-comma`. Recommended landing order is
mode 0 → 2 → 3 → 4; mode 1 falls out of mode 2 trivially.

### Picking a mode

| Mode | Default | Cost to implement | Flags |
| --- | --- | --- | --- |
| 0 — denylist only | opt-in | smallest | known-conditional macros only |
| 1 — blanket | opt-in | small | every non-trivial macro arg |
| 2 — allowlist + denylist | **on** | small | denylist always; unknown configurable |
| 3 — mode 2 + bypass | opt-in (flag) | small + recursion | mode 2 minus pure-call shapes |
| 4 — matcher-based | opt-in | large | mode 2 plus learned `macro_rules!` |

Modes 0, 1, and 2 share the same visitor and differ only in
the name-lookup table. Mode 3 adds one extra predicate. Mode 4
adds the matcher walker. The recommended implementation order
matches the table: mode 0 is the smallest landable step, mode
4 is the largest.

## What to lint

For every macro invocation:

1. Check the invocation's `Delimiter`. If it is `Brace`, skip —
   curly-brace invocations are out of scope.
2. Resolve the macro `DefId`.
3. Consult `ignore`. If the path matches, skip. `ignore` wins
   over both the denylist and the allowlist; it exists for
   per-project opt-outs of curated entries.
4. Apply the configured `mode`:
   - `denylist_only`: match against the denylist
     (`debug_assert!`, `debug_assert_eq!`, `debug_assert_ne!`,
     `cfg!`, plus `extra_denylist`). Continue to step 5 only
     on a hit.
   - `blanket`: continue to step 5 for every macro not on the
     allowlist (`extra_allowlist` only — there is no built-in
     allowlist in this mode, by design).
   - `allowlist_denylist` (default): denylist hit → continue.
     Allowlist hit → skip. Otherwise consult
     `unknown_macro_policy`: `allow` (default) → skip,
     `deny` → continue.
   - `matcher_based`: denylist / allowlist as above; for
     unknown macros, walk the matcher per mode 4. Result is
     either "treat as allowlisted" (continue: skip) or
     "treat as denylisted" (continue: lint).
5. Walk the invocation's *top-level* argument list. The lint
   only inspects top-level expressions — an argument that
   itself contains a non-trivial sub-expression is the
   author's choice, and recursing into nested macros opens
   the door to false positives. Top-level argument boundaries
   are recovered from the invocation token stream the same
   way [`macro-trailing-comma`](./macro-trailing-comma.md)
   does it (track delimiter nesting; split on top-level
   commas; skip top-level `;` and `=>`).
6. For each top-level argument, parse the token stream as an
   expression. Skip arguments that don't parse as a single
   expression (a `name: type` argument shape, a path-only
   argument like `serde!(foo = bar)`, etc.) — those are
   syntactic positions the macro author chose, not normal
   value arguments.
7. Classify the expression with the trivial / non-trivial
   split above. If non-trivial, and the `expression_bypass`
   knob is enabled, apply the bypass recursion before
   deciding.
8. Emit a diagnostic on the non-trivial argument's span. The
   suggested rewrite is a `let` binding immediately before
   the macro call, with the binding name derived from the
   expression (a fallback identifier such as `binding` is
   used when no better name is available).

The autofix is `Applicability::MaybeIncorrect`. Inserting a
`let` binding adds a new name to the enclosing scope, which
may shadow an existing name or be shadowed by a later
binding. The rewrite is mechanically correct but worth a
human glance.

## Examples

### The motivating bug

```rust
// Bad — release mode skips `insert` entirely
debug_assert_eq!(my_set.insert(new_item), None, "duplicate insert");

// Good
let was_new = my_set.insert(new_item).is_none();
debug_assert!(was_new, "duplicate insert");
```

### Trivial arguments stay inline

```rust
// Accepted — `count` is a path, `MAX_RETRIES` is a const,
// `&buffer` is a reference to a path. None of them have
// side effects.
debug_assert_eq!(count, MAX_RETRIES, "expected {MAX_RETRIES} retries");
debug_assert!(buffer.is_empty(), "buffer must start empty");
//            ^^^^^^^^^^^^^^^^^ method call → flagged under
//                              the strict default;
//                              accepted under
//                              `expression_bypass = true`
//                              because `is_empty` takes only
//                              `&self`.
```

In practice nearly every `debug_assert!` argument is a
boolean-returning method or function call. Without
`expression_bypass = true` the lint flags essentially every
reasonable `debug_assert!` invocation, which is not a
recommended deployment shape. Projects that adopt the default
denylist for the `insert`-style bug above almost always want
the bypass on at the same time; the two knobs are paired in
typical configurations even though they default to opposite
positions out of the box.

### Allowlisted macros pass through

```rust
// Accepted under default config — `format!` is on the
// curated allowlist; arguments are evaluated exactly once.
let msg = format!("retrying {} ({} failures)", endpoint, count.fetch_add(1, Ordering::Relaxed));
```

### Array-like invocation is also in scope

```rust
// Accepted under default config — vec! is on the curated
// allowlist; each element is evaluated exactly once.
let xs = vec![compute(), compute(), compute()];

// Flagged under blanket mode — every non-trivial argument is
// a candidate, allowlist or not.
let xs = vec![compute(), compute(), compute()];

// Good (blanket mode rewrite)
let a = compute();
let b = compute();
let c = compute();
let xs = vec![a, b, c];
```

### Curly-brace invocation is out of scope

```rust
// Skipped — curly-brace invocation. The DSL contract is the
// macro's, not the call site's.
thread_local! {
    static COUNTER: Cell<u32> = Cell::new(compute_initial());
}
```

### Multiple-evaluation trap

```rust
macro_rules! double_use {
    ($e:expr) => { $e + $e };
}

// Bad — `next` is consumed twice. The current value plus the
// next value, not "doubled current".
let total = double_use!(iter.next().unwrap());

// Good
let v = iter.next().unwrap();
let total = double_use!(v);
```

This is the kind of misuse mode 4 (matcher-based) catches
automatically: the matcher walker sees `$e` referenced twice in
the RHS and flags every non-trivial argument to `double_use!`
even though the macro is otherwise unknown to the lint.

### Procedural macro stays in user-config territory

```rust
// Whether this is safe depends on the proc macro's expansion,
// which the lint cannot read. The user adds `tracing::info`
// to `extra_allowlist` once, project-wide.
tracing::info!(latency = stopwatch.elapsed().as_millis(), "done");
```

## Configuration

```toml
[macro_argument_binding]
# Set to false to disable the rule entirely.
enabled = true

# Eligibility mode. Defaults to "allowlist_denylist".
#   "denylist_only"     — flag only the curated denylist
#   "blanket"           — flag everything not on extra_allowlist
#   "allowlist_denylist" — flag denylist hits + unknowns per policy
#   "matcher_based"     — allowlist_denylist + matcher walking
mode = "allowlist_denylist"

# Behaviour for macros that match neither the denylist nor the
# allowlist under "allowlist_denylist" mode. `allow` (default)
# silently skips them; `deny` treats them as denylisted.
# Ignored under other modes.
unknown_macro_policy = "allow"

# When true, accept calls and method calls whose arguments are
# themselves trivial — even on denylisted macros. Default off;
# turn on if the lint is too noisy with read-only accessor
# calls inside `debug_assert*`.
expression_bypass = false

# Macros added to the built-in denylist. Each entry is a
# fully-qualified macro path (no trailing `!`) or a bare macro
# name to match by final segment only.
extra_denylist = [
  # "my_crate::sometimes_evaluates",
]

# Macros added to the built-in allowlist. Same path syntax as
# extra_denylist. Use for third-party macros the project
# trusts to evaluate each argument exactly once.
extra_allowlist = [
  # "tracing::info",
  # "tracing::debug",
]

# Macros to skip entirely, regardless of which list they would
# otherwise hit. Use for project-internal macros whose
# arguments are intentionally expressions (the author knows
# the matcher).
ignore = [
  # "my_crate::ad_hoc",
]
```

## Implementation notes

- `EarlyLintPass::check_mac` over `ast::MacCall`. The early
  pass sees the raw invocation token stream, which the lint
  needs to split into top-level arguments before any
  expansion has happened. The same `#[expect]`-fulfilment
  workaround that
  [`macro-trailing-comma`](./macro-trailing-comma.md) uses
  (park spans in a process-static queue, emit from a late
  pass that walks HIR) applies here.
- Macro path resolution: `MacCall::path` resolves to a `Res`;
  use the resolved `DefId` for the name-lookup. Bare-name
  matching falls back to the path's final segment so
  `extra_allowlist = ["my_macro"]` works without forcing the
  user to spell out the crate path.
- Argument splitting: walk `MacCall::args.tokens` tracking
  delimiter nesting and split on top-level commas. Reuse the
  helper that
  [`macro-trailing-comma`](./macro-trailing-comma.md)
  introduces (or factor it out the other way around,
  whichever rule lands first).
- Per-argument re-parse: each top-level argument is a token
  stream; reparse it as an expression with `rustc_parse`'s
  `Parser::parse_expr` (or the equivalent
  restriction-respecting helper if the surrounding context
  needs it). Arguments that fail to parse as an expression
  are skipped (they are not value-shaped — `name = value`,
  `name: type`, and similar syntactic positions that some
  macros consume).
- Trivial / non-trivial predicate: a `match` on
  `ast::ExprKind` covering `Lit`, `Path`, `AddrOf`, `Field`,
  `Index`, `Unary(Deref, _)`, `Cast`, and the trivial-base
  recursions. Default to non-trivial for any unrecognised
  variant — false positives are better than false negatives
  here.
- Expression-side bypass (mode 3): a recursive descent
  through `ExprKind::Call` and `ExprKind::MethodCall`. The
  callee / receiver must itself be trivial; each argument
  must be trivial-after-bypass. A bypass match implies the
  call is "as safe as an accessor".
- Matcher walker (mode 4): reuse the `take_*` combinator
  scaffold introduced for
  [`macro-trailing-comma`](./macro-trailing-comma.md)'s
  matcher-based mode. The two rules ask different questions
  of the matcher (trailing-comma-optional vs.
  capture-evaluation-count) but consume the same token
  grammar and the same cross-crate access path
  (`tcx.hir_node_by_def_id` for local macros,
  `tcx.cstore_untracked()` for dependency macros). Factor
  the matcher access into a crate-internal module and have
  both rules consume it.
- Suggested rewrite: render `let <name> = <expr>;\n<indent>`
  immediately before the macro invocation. Name derivation
  is a best-effort heuristic — pick the receiver / callee
  identifier when the expression is a method or function
  call, fall back to `binding` otherwise. The fix is
  `Applicability::MaybeIncorrect` because the inserted name
  may shadow an existing binding or change scoping.

### Difficulty

**Mode 0 / 1 / 2: easy.** A name-set lookup, a top-level
argument splitter (reused from `macro-trailing-comma`), and a
syntactic predicate on `ast::ExprKind`. The three modes share
the entire pipeline and differ only in which lookup table the
name resolution consults.

**Mode 3: easy.** One extra predicate layered on the
expression classifier; recursion bottoms out at the trivial
cases.

**Mode 4: hard.** Same matcher-walking infrastructure as
[`macro-trailing-comma`](./macro-trailing-comma.md)'s
matcher-based mode, plus capture-occurrence counting and the
nested-repetition case analysis. Recommended landing order is
modes 0-3 in a single PR (they share the pipeline), then mode
4 as a follow-up that also lights up `macro-trailing-comma`'s
own matcher-based detection.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in
  this catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Severity

Warn. The denylist defaults — `debug_assert*` and `cfg!` —
flag a genuine correctness bug. Promoting the lint to deny
crate-wide via
`#![deny(perfectionist::macro_argument_binding)]` is viable
but presumes the project has already turned
`expression_bypass = true` on (or has narrowed the denylist):
under the strict default, every `debug_assert!` invocation
with a non-trivial argument fires, which is the majority of
them, and deny would refuse to compile the project.

## Interaction with sibling rules

- [`macro-trailing-comma`](./macro-trailing-comma.md) shares
  the top-level argument splitter and (eventually) the
  declarative-macro matcher walker. The two rules ask
  different questions of the same invocation; both register
  for `ast::MacCall` and both restrict themselves to
  function-like and array-like delimiters. Factor the shared
  scanner into a crate-internal helper so neither rule grows
  its own copy.
- [`format-macro-wrap`](./format-macro-wrap.md) and
  [`print-macro-split`](./print-macro-split.md) operate on
  the *template literal* inside their target macros and
  treat the surrounding macro as a known-safe call. Those
  rules' target macros are all on this rule's default
  allowlist, so the two never disagree about whether a
  given invocation needs intervention.
