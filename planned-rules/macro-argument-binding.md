# `macro_argument_binding`

**Source:** project convention. The motivating bug:

```rust
debug_assert_eq!(my_map.insert(key, value), None, "Something went wrong! `key` wasn't new");
```

In debug builds this works: `insert` runs, returns the previous
value (or `None` if the key was new), and the assertion panics
if `key` was already present. In release builds the
`debug_assertions` cfg is off, the conditional that
`debug_assert_eq!` expands to folds to `if false { ... }`, and
the body is dead-code-eliminated. The argument expressions are
*not evaluated*: `insert` never runs, `(key, value)` is silently
dropped, and `my_map` ends the function in a different state
from the one the author intended. The bug only surfaces under
`--release`.

The fix is to bind the call to a `let` first, then pass the
binding:

```rust
let ejected = my_map.insert(key, value);
debug_assert_eq!(ejected, None, "Something went wrong! `key` wasn't new");
```

`debug_assert*` is the famous offender, but the trap is general:
a function-like or array-like macro may evaluate any top-level
argument zero, one, or many times depending on its matcher.
Functions guarantee exactly-once evaluation per argument; macros
do not, even when the call shape looks identical.

## Why is this bad?

Code containing such an invocation behaves differently between
debug and release builds (or between any two configurations the
macro is conditional on), and the difference is invisible at the
call site. A duplicate insertion silently succeeds in release; a
`?` expression that should have short-circuited an error never
runs; an iterator's `.next()` quietly advances in one build
configuration but not another. These aren't stylistic
preferences — they're bugs.

The trap covers two shapes:

- **Conditional evaluation** (`debug_assert*`, log macros below
  the configured filter level, custom `#[cfg(test)]` wrappers).
  Side effects vanish in some build configurations.
- **Repeated evaluation** (anything implementing a syntactic
  transformation that expands its capture more than once —
  `min!`/`max!`-style macros, retry-loop macros). A
  side-effecting expression repeated produces wrong results.

A `let` binding upstream of the macro call removes the
ambiguity: the expression evaluates exactly once, regardless of
what the macro does with its captures.

## Statement

For a function-like (`name!(...)`) or array-like (`name![...]`)
macro invocation:

- If the macro is on the **deny list**, every non-trivial
  top-level argument is flagged.
- If the macro is on the **allow list**, the argument shape is
  unconstrained.
- For every other macro, behaviour depends on the configured
  `mode` (see "Eligibility modes" below).

Curly-brace macro invocations (`name! { ... }`) are out of
scope — they're conventionally DSL bodies
(`thread_local! { ... }`, `quote! { ... }`, `html! { ... }`)
where the evaluation contract is the macro's, not the call
site's. The call-site delimiter is the only signal the lint
uses: the same `macro_rules!` accepts all three call shapes,
and the definition site doesn't fix which one the author chose
at any given call site.

### What counts as a "non-trivial" argument

The lint accepts any argument whose outermost shape is one of:

- A literal (`42`, `"hello"`, `true`, `'a'`).
- A path resolving to a `const`, `static`, local binding,
  function name, or unit / tuple variant (`MAX`, `count`,
  `Foo::BAR`, `Result::Ok`).
- A reference to a trivial expression (`&count`, `&mut buffer`).
- A field access or tuple-index on a trivial base
  (`config.threshold`, `point.0`).
- An index `base[index]` where both `base` and `index` are
  themselves trivial (`buffer[0]`, `lookup[Foo::KEY]`).
- A unary deref of a trivial expression (`*ptr`).
- A trivial expression annotated with a type (`x as u64`).

Everything else is non-trivial: function and method calls, `?`,
`.await`, macro invocations, blocks, control-flow expressions,
assignments, and any binary or range expression whose operands
are non-trivial. The classification is purely syntactic — `const
fn` calls and other "morally pure" expressions are non-trivial;
hoist them to a `const` if they need to appear inline.

## Eligibility modes

Four modes ordered by implementation cost. The default is
`allow_and_deny`.

### Mode 0 — `deny_only`

Flag only invocations of the curated deny list (`debug_assert!`,
`debug_assert_eq!`, `debug_assert_ne!`) plus any `deny_extra`
entries. Every other macro is silently accepted.

The smallest landable rule. Pick this when a project wants the
`debug_assert*` footgun caught without auditing third-party
macros.

### Mode 1 — `blanket`

Flag every function-like or array-like invocation that carries
a non-trivial top-level argument, regardless of macro. Add
specific exceptions to `allow_extra`; there is no built-in
allow list in this mode.

The maximum-paranoia stance. Not the default because
`format!("hello {name}", compute())` is fine in practice and
flagging every macro invocation is exhausting.

### Mode 2 — `allow_and_deny` (default)

Three name-set lookups decide each invocation:

1. **Deny-list hit** (`debug_assert*` plus `deny_extra`) → flag
   every non-trivial argument.
2. **Allow-list hit** (the curated set below plus `allow_extra`)
   → accept unconditionally.
3. **Neither** → flag every non-trivial argument.

The default allow list tracks the curated set in
[`macro-trailing-comma`](./macro-trailing-comma.md), with the
conditional-evaluation families (`log::*`, `tracing::*`)
removed: `format!`, `format_args!`, `print!`, `println!`,
`eprint!`, `eprintln!`, `write!`, `writeln!`, `vec!`,
`panic!`, `unimplemented!`, `todo!`, `unreachable!`,
`assert!`, `assert_eq!`, `assert_ne!`, `matches!`, `dbg!`,
`anyhow!`, and similar.

Flagging unlisted macros by default is deliberate: the rule
isn't useful if every unrecognised proc macro gets a free pass.
Projects extend `allow_extra` with the third-party macros they
trust to evaluate each argument exactly once.

### Mode 3 — `matcher_based`

Layers on top of mode 2. The allow list and deny list are
consulted first; the matcher walk runs only on `macro_rules!`
macros that would otherwise be unknown.

For an eligible declarative macro, determine which arm matches
the invocation. For each `$name:expr` capture in that arm,
count occurrences of `$name` in the expansion:

- Exactly one, not nested inside any `$( ... )*` / `$( ... )+` /
  `$( ... )?` repetition → the argument is evaluated exactly
  once. Treat the invocation as allow-listed.
- Zero, two or more, or any occurrence inside a repetition or
  conditional fragment → flag the invocation.

Procedural macros are not walkable; the lint falls back to the
mode-2 verdict (flag, by default).

The most expensive mode to implement. Ship modes 0-2 first; mode
3 can share matcher-walking infrastructure with the matcher-
based eligibility check planned for
[`macro-trailing-comma`](./macro-trailing-comma.md).

## What to lint

For every macro invocation:

1. Skip if the delimiter is `Brace`.
2. Read `MacCall::path` segments. The pre-expansion pass runs
   before name resolution, so this is a raw `rustc_ast::Path`
   matched syntactically against configuration entries.
3. If the path matches an `ignore` entry, skip.
4. Apply the configured mode to decide whether to inspect the
   arguments (see "Eligibility modes").
5. Walk the invocation's top-level argument list using the
   same token-stream handling as
   [`macro-trailing-comma`](./macro-trailing-comma.md): track
   delimiter nesting and split on top-level commas. Skip the
   whole invocation if the top-level token stream uses `;` as
   a separator (`vec![v; count]`); top-level `=>` is ordinary
   content and walked through.
6. For each top-level argument, parse the token stream as an
   expression. Skip arguments that don't parse as a single
   expression (`name: type`, `name = value`, etc. are syntactic
   positions the macro author chose).
7. Classify the expression with the trivial / non-trivial split.
   If trivial, accept; if non-trivial, emit a diagnostic
   suggesting a `let` binding immediately before the macro
   call.

The diagnostic is informational only; no autofix is supplied
because the right binding name varies per site and the rewrite
introduces a new name in the enclosing scope.

## Examples

### The motivating bug

```rust
// Bad — release skips `insert` entirely
debug_assert_eq!(my_map.insert(key, value), None, "duplicate key");

// Good
let ejected = my_map.insert(key, value);
debug_assert_eq!(ejected, None, "duplicate key");
```

### Trivial arguments stay inline

```rust
// Accepted — `count`, `MAX_RETRIES`, `&buffer` are all trivial.
debug_assert_eq!(count, MAX_RETRIES, "expected {MAX_RETRIES} retries");
```

### Allow-listed macros pass through

```rust
// Accepted — `format!` is on the curated allow list; arguments
// are evaluated exactly once.
let msg = format!("retrying {} ({} failures)", endpoint, count.fetch_add(1, Ordering::Relaxed));
```

### Array-like invocation is in scope

```rust
// Accepted under default config — `vec!` is on the allow list.
let xs = vec![compute(), compute(), compute()];
```

```rust
// Flagged under blanket mode — every non-trivial argument is
// a candidate, allow list or not.
let xs = vec![compute(), compute(), compute()];

// Good (blanket-mode rewrite)
let a = compute();
let b = compute();
let c = compute();
let xs = vec![a, b, c];
```

### Multiple-evaluation trap

```rust
macro_rules! double_use {
    ($e:expr) => { $e + $e };
}

// Bad — `iter.next()` runs twice. `total` ends up as
// `current_item + next_item`, not `2 * current_item`.
let total = double_use!(iter.next().unwrap());

// Good
let v = iter.next().unwrap();
let total = double_use!(v);
```

Mode 3 catches this automatically: `$e` is referenced twice in
the expansion, so the macro fails the exactly-once check.

### Procedural macros require explicit configuration

```rust
// Whether this is safe depends on the proc macro's expansion,
// which the lint cannot inspect. A project adds
// `serde_json::json` to `allow_extra` once project-wide after
// confirming each argument is evaluated exactly once.
let payload = serde_json::json!({ "id": next_id(), "ts": now() });
```

## Configuration

```toml
[macro_argument_binding]
# Set to false to disable the rule entirely.
enabled = true

# Eligibility mode. Defaults to "allow_and_deny".
mode = "allow_and_deny"

# Macros added to the built-in deny list. Each entry is a
# fully-qualified macro path (no trailing `!`) or a bare macro
# name to match by final segment only.
deny_extra = [
  # "my_crate::sometimes_evaluates",
]

# Macros added to the built-in allow list.
allow_extra = [
  # "serde_json::json",
]

# Macros to skip entirely, regardless of which list they would
# otherwise hit.
ignore = [
  # "my_crate::ad_hoc",
]
```

## Implementation notes

- `EarlyLintPass::check_mac` over `ast::MacCall`. The
  `#[expect]`-fulfilment machinery (park spans pre-expansion,
  emit from a late HIR pass) follows the pattern documented
  in [`macro-trailing-comma`](./macro-trailing-comma.md).
- Macro path matching is syntactic over `path.segments` — the
  pre-expansion pass runs before name resolution, so there's
  no `Res` / `DefId` to consult. Reuse
  [`macro-trailing-comma`](./macro-trailing-comma.md)'s
  `matches_any` / `entry_matches` helpers; single-segment
  entries tail-match the path, multi-segment entries
  tail-match the segment sequence.
- Argument splitting walks `MacCall::args.tokens`, tracks
  delimiter nesting, and splits on top-level commas. Share
  the splitter with `macro-trailing-comma`.
- Per-argument expression re-parse uses `rustc_parse`'s
  `Parser::parse_expr` (or the equivalent restriction-
  respecting helper for the surrounding context).
- Trivial/non-trivial predicate: a `match` on `ast::ExprKind`
  over the seven trivial variants (`Lit`, `Path`, `AddrOf`,
  `Field`, `Index`, `Unary(Deref, _)`, `Cast`) with recursive
  triviality checks on the sub-expressions. Default any
  unrecognised variant to non-trivial.
- Matcher walker (mode 3): builds on the matcher-access
  infrastructure
  [`macro-trailing-comma`](./macro-trailing-comma.md)
  introduces for its own matcher-based eligibility check.

### Difficulty

**Modes 0 / 1 / 2: easy.** A syntactic name-set lookup, a
top-level argument splitter, and the trivial / non-trivial
predicate. The three modes differ only in which lookup table
the matcher consults.

**Mode 3: hard.** Same matcher-walking infrastructure as
[`macro-trailing-comma`](./macro-trailing-comma.md)'s
matcher-based mode, plus capture-occurrence counting. Ship
modes 0-2 first; mode 3 as a follow-up.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions, in particular the lint-name
  namespacing under `perfectionist::*`.

## Severity

Warn.

## Interaction with sibling rules

- [`macro-trailing-comma`](./macro-trailing-comma.md) shares
  the top-level argument splitter and (eventually) the
  declarative-macro matcher walker. Both register for
  `ast::MacCall` and both restrict themselves to function-like
  and array-like delimiters.
- [`format-macro-wrap`](./format-macro-wrap.md) and
  [`print-macro-split`](./print-macro-split.md) operate on the
  *template literal* inside their target macros. Those rules'
  target macros are all on this rule's default allow list.
