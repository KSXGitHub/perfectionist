# `macro_argument_binding`

## Status

Partially implemented. Modes 0-2 (`deny_only`, `blanket`,
`allow_and_deny`) ship today, along with the `enabled`,
`deny_extra`, `allow_extra`, `ignore`, `extra_pure_methods`,
`ignore_pure_methods`, `extra_pure_macros`, and
`ignore_pure_macros` knobs. The lint emits diagnostics with
a `let`-binding hint (no autofix, by design — the binding name
varies per site).

Still pending:

- **Mode 3 (`matcher_based`).** The mode value is not accepted by
  the configuration parser yet; a `dylint.toml` that names it
  fails to deserialise. The matcher-walking infrastructure is
  shared with the equivalent eligibility check planned for
  `macro-trailing-comma`; both will land together.
- **Range expressions over pure operands.** The spec couples
  range-expression purity to operand purity the same way
  it does for binary expressions, but the walker only recognises
  binary chains; `start..end` and `start..=end` over pure
  operands still fall through as impure. Extending
  `take_pure_expression` to optionally consume a range tail is
  a small follow-up.
- **Cast suffix beyond path-shaped types.** The pure-expression
  predicate currently recognises `expr as Path` (e.g., `x as u64`,
  `x as my::Type`) but treats `expr as &Path`, `expr as *const T`,
  and other non-path type forms as impure. Expanding the
  type recogniser is a small, additive change.
- **Turbofish in path arguments.** A path with explicit generics
  (`Vec::<u32>::new`, `Some::<u32>`) is parsed as `path-segment` plus
  `::<...>` plus more segments; the current path walker only consumes
  `::ident` runs and so falls through to impure on the turbofish.
  These should be pure per the spec's intent ("a path resolving to
  a function name, or unit / tuple variant"); extend `take_path_tail`
  to consume an optional `::<...>` token-tree per segment.
- **Keyword idents as path starts.** The pure-atom matcher's
  `Ident(_, _)` branch dispatches to the path walker regardless of
  whether the ident is a valid path-start keyword (`self`, `Self`,
  `super`, `crate`, the empty set otherwise). Reserved keywords like
  `let`, `if`, `match`, `for`, `while` are accepted as path heads
  and the resulting "pure path" leaves an unexpected tail in the
  suffix walk, which still bottoms out as impure — so the lint
  classification is correct by coincidence. Tighten `take_pure_atom`
  to reject reserved-keyword idents so the right door owns the
  decision.

The "What to lint" pipeline below applies to the implemented
modes as written. The remainder of this file is the active spec
for the unimplemented portion.

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

- If the macro is on the **deny list**, every impure
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

### What counts as a "impure" argument

The lint accepts any argument whose outermost shape is one of:

- A literal (`42`, `"hello"`, `true`, `'a'`).
- A path resolving to a `const`, `static`, local binding,
  function name, or unit / tuple variant (`MAX`, `count`,
  `Foo::BAR`, `Result::Ok`).
- A reference to a pure expression (`&count`, `&mut buffer`).
- A field access or tuple-index on a pure base
  (`config.threshold`, `point.0`).
- An index `base[index]` where both `base` and `index` are
  themselves pure (`buffer[0]`, `lookup[Foo::KEY]`).
- A unary deref of a pure expression (`*ptr`).
- A pure expression annotated with a type (`x as u64`).
- The unit literal `()`, a parenthesised pure expression
  (`(x)`), or a tuple whose every element is pure
  (`(a, b)`, `(a,)`).
- The empty array literal `[]`, an array literal whose every
  element is pure (`[a, b, c]`, optional trailing comma), or an
  array-repeat `[expr; count]` whose `expr` and `count` are both
  pure (`[0; 4]`, `[MAX; LEN]`). Array literals introduce no
  side effect beyond their elements, and the array-repeat form
  evaluates `expr` at most once at runtime, so neither shape
  benefits from a let-bind rewrite when its parts are already
  pure. The indexing suffix `base[index]` is handled by the
  postfix walker and is unaffected by this carve-out.
- A binary chain whose every operand is pure and whose
  every operator is side-effect-free in the syntactic sense:
  the arithmetic operators (`+`, `-`, `*`, `/`, `%`), the
  bitwise operators (`&`, `|`, `^`, `<<`, `>>`), the
  comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`),
  and the short-circuit operators (`&&`, `||`). `a <= b`,
  `count + offset`, `flags & MASK == 0` are all pure when
  the operands are.
- A zero-argument method call `expr.method()` on a pure
  base, where `method` is in the curated pure-getter set
  (`len`, `is_empty`, `as_str`, `as_bytes`, `as_ref`, `as_mut`,
  `as_deref`, `as_slice`) or in the project's
  `extra_pure_methods`. `vec.len()`, `s.is_empty()`,
  `opt.as_ref()` evaluate the same way no matter how many
  times the macro touches them, so the let-bind rewrite
  would only force the call to run in release builds for
  no benefit. Method calls with arguments, generic method
  calls, and method names outside the configured set stay
  impure: `map.insert(k, v)`, `iter.next()`,
  `vec.try_into::<Foo>()` still flag.
- A function-like or array-like invocation `name!(...)` /
  `name![...]` of a curated pure macro: `concat!`, `env!`,
  `option_env!`, `include_str!`, `include_bytes!`,
  `stringify!`, `cfg!`, `line!`, `column!`, `file!`,
  `module_path!`, plus anything in `extra_pure_macros`.
  These expand to compile-time constants (a literal, a
  `&'static str`, a `bool`, a span marker) with no runtime
  side effect, so passing one to a surrounding `debug_assert*`
  or any other macro does not introduce the
  evaluate-once-vs.-zero hazard the rule is built to catch.
  Match is tail-segment-keyed: `env!`, `std::env!`, and
  `::core::env!` all match the `"env"` entry. The macro's
  body contents are not inspected. The justification isn't
  that the input shape is restricted — `stringify!` accepts
  arbitrary tokens and `cfg!` accepts cfg predicates — but
  that none of these macros evaluates a runtime user
  expression in the first place, so there is nothing for
  the surrounding rule to drop or duplicate regardless of
  what's inside. Curly-delimited inner macros (`name! { ... }`)
  do *not* qualify; the rule treats those as DSL bodies, the
  same way it treats the outer call's curly form.

Everything else is impure: function and method calls, `?`,
`.await`, macro invocations, blocks, control-flow expressions,
assignments, range expressions, and any binary expression whose
operands are impure. The classification is purely
syntactic — `const fn` calls and other "morally pure"
expressions are impure; hoist them to a `const` if they
need to appear inline.

The binary-chain rule reflects an important consequence for
`debug_assert*`: a side-effect-free comparison of pure
operands evaluates the same way regardless of how many times the
macro touches it, and the lint's `let`-binding hint would
*force* the comparison to evaluate even in release builds. The
pure-chain carve-out keeps the lint focused on the genuine
hazard (side-effecting expressions like `map.insert(k, v)`
passed where the macro might drop them) and away from the noise
case (comparing two locals).

### Caveats of the pure-getter postfix rule

The pure-getter rule is **syntactic, name-based, type-blind**. A
third-party type that defines an inherent method named
`is_empty`, `len`, `as_bytes`, `as_ref`, … and that performs
observable side effects in that method will be incorrectly
accepted as pure. The curated list is restricted to names
whose pure-getter convention is essentially universal across the
ecosystem, but the lint cannot prove the convention holds for
any given call site. Projects that hit this corner can drop
specific names from the built-in set via the
`ignore_pure_methods` knob — for example, a project that
wraps `as_ref` in a non-pure implementation can put `"as_ref"`
in `ignore_pure_methods` and the lint will flag every
`.as_ref()` call as a method call again.

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
an impure top-level argument, regardless of macro. Add
specific exceptions to `allow_extra`; there is no built-in
allow list in this mode.

The maximum-paranoia stance. Not the default because
`format!("hello {name}", compute())` is fine in practice and
flagging every macro invocation is exhausting.

### Mode 2 — `allow_and_deny` (default)

Three name-set lookups decide each invocation:

1. **Deny-list hit** (`debug_assert*` plus `deny_extra`) → flag
   every impure argument.
2. **Allow-list hit** (the curated set below plus `allow_extra`)
   → accept unconditionally.
3. **Neither** → flag every impure argument.

The default allow list has three parts.

The first part overlaps with
[`macro-trailing-comma`](./macro-trailing-comma.md)'s built-in
set, minus the conditional-evaluation families (`log::*`,
`tracing::*`) that *do* drop arguments below the configured
filter level: `format!`, `format_args!`, `print!`,
`println!`, `eprint!`, `eprintln!`, `write!`, `writeln!`,
`vec!`, `panic!`, `unimplemented!`, `todo!`, `unreachable!`,
`assert!`, `assert_eq!`, `assert_ne!`, `matches!`, `dbg!`,
`anyhow!`, and similar. These are runtime macros whose
matchers promise exactly-once evaluation per top-level
argument.

The second part adds `core` / `std` macros whose top-level
argument simply isn't a runtime expression — `stringify!`
takes a token sequence, `cfg!` takes a cfg predicate, the
`env!` / `option_env!` / `include_str!` / `include_bytes!` /
`include!` / `is_x86_feature_detected!` family takes a
string literal, the `line!` / `column!` / `file!` /
`module_path!` family takes no argument, and
`compile_error!` aborts compilation — so there is no
once-vs.-zero hazard for the rule to flag.
`is_x86_feature_detected!` does perform a cached CPU check
at runtime, but the lookup runs without any user-side
argument evaluation, so it sits comfortably in the same
group.

The third part collects third-party macros whose matchers are
known to evaluate every top-level argument exactly once before
forwarding it, so the rule's once-vs.-zero hazard does not apply.
The current entries are the `insta` snapshot-assertion family —
`assert_snapshot!`, `assert_debug_snapshot!`,
`assert_display_snapshot!`, `assert_compact_debug_snapshot!`,
`assert_yaml_snapshot!`, `assert_json_snapshot!`,
`assert_compact_json_snapshot!`, `assert_ron_snapshot!`,
`assert_csv_snapshot!`, `assert_toml_snapshot!`, and
`assert_binary_snapshot!` — but the group is open-ended; further
crates can be added as their matchers are vetted.
`assert_display_snapshot!` is deprecated upstream in favour of
`assert_snapshot!` but is retained for projects on older `insta`
releases.

`macro-trailing-comma`'s built-in list is intentionally
narrower than this one; the two lists are not kept in
lockstep.

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
   expression — including positional operator markers
   (`debug_assert_op_expr!(a, ==, b)`), assignment-shaped
   matchers (`make_const!(NAME = 'x')`, `bump!(counter += 1)`),
   `name: type` ascription-shaped matchers, `Type => body`
   match-arm DSLs, `lhs -> rhs` arrow-paired matchers
   (`link!("src" -> "dst")`), and `name in name`-style separators
   (`for_each!(x in iter, ...)`).
   All are syntactic positions the macro author chose, and the
   let-bind rewrite the rule would propose is meaningless for
   the macro's matcher arm.
7. Classify the expression with the pure / impure split.
   If pure, accept; if impure, emit a diagnostic
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

### Pure arguments stay inline

```rust
// Accepted — `count`, `MAX_RETRIES`, `&buffer` are all pure.
debug_assert_eq!(count, MAX_RETRIES, "expected {MAX_RETRIES} retries");
```

### Allow-listed macros pass through

```rust
// Accepted — `format!` is on the curated allow list; arguments
// are evaluated exactly once.
let msg = format!("retrying {} ({} failures)", endpoint, count.fetch_add(1, Ordering::Relaxed));
```

### Compile-time `core` / `std` macros pass through

```rust
// Accepted — `concat!`, `env!`, `include_str!`, and the rest of
// the compile-time family are on the allow list, and their
// expansion is itself a literal so they also count as pure
// atoms when used inside another macro. Both rules together
// make these idioms invisible to the lint:
let msg = concat!("home: ", env!("HOME"));
debug_assert_eq!(env!("EXPECTED"), include_str!("expected.txt"));
```

### Array-like invocation is in scope

```rust
// Accepted under default config — `vec!` is on the allow list.
let xs = vec![compute(), compute(), compute()];
```

```rust
// Flagged under blanket mode — every impure argument is
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

# Zero-argument method names treated as pure postfixes on a
# pure base, in addition to the built-in set (`len`,
# `is_empty`, `as_str`, `as_bytes`, `as_ref`, `as_mut`,
# `as_deref`, `as_slice`). Add project-specific pure getters
# here so `debug_assert!(value.my_cached_getter() <= limit)`
# stops flagging.
extra_pure_methods = [
  # "my_cached_getter",
]

# Method names to drop from the pure-method list, even if they
# appear in the built-in defaults or in `extra_pure_methods`.
# Checked after the merge, so this knob always wins. Useful for
# opting back into linting on a default entry the project does
# not consider pure.
ignore_pure_methods = [
  # "as_ref",
]

# Macro names treated as pure atoms when they appear as
# arguments to another macro, in addition to the built-in set
# (`concat`, `env`, `option_env`, `include_str`, `include_bytes`,
# `stringify`, `cfg`, `line`, `column`, `file`, `module_path`).
# Add project-specific compile-time macros here so that, e.g.,
# `debug_assert_eq!(literal_table!(KEY), expected)` stops flagging
# the inner call.
extra_pure_macros = [
  # "literal_table",
]

# Macro names to drop from the pure-macro list, even if they
# appear in the built-in defaults or in `extra_pure_macros`.
# Checked after the merge, so this knob always wins.
ignore_pure_macros = [
  # "cfg",
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
- Pure/impure predicate: a `match` on `ast::ExprKind`
  over the seven pure variants (`Lit`, `Path`, `AddrOf`,
  `Field`, `Index`, `Unary(Deref, _)`, `Cast`) with recursive
  purity checks on the sub-expressions. Default any
  unrecognised variant to impure.
- Matcher walker (mode 3): builds on the matcher-access
  infrastructure
  [`macro-trailing-comma`](./macro-trailing-comma.md)
  introduces for its own matcher-based eligibility check.

### Difficulty

**Modes 0 / 1 / 2: easy.** A syntactic name-set lookup, a
top-level argument splitter, and the pure / impure
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
