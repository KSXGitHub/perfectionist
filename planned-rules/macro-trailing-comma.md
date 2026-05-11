# `macro_trailing_comma`

**Source:** project convention. Fills a gap that
`rustfmt`'s `trailing_comma = "Vertical"` (the default) leaves
open: rustfmt rewrites function-call and struct-literal argument
lists — adding a trailing comma when the list is broken across
multiple source lines, removing one when it collapses onto a
single line — but it does not touch macro invocations. The
conservative default exists because a macro's matcher can make
the trailing comma load-bearing, and rustfmt has no way to know.

This lint reinstates the same comma policy for macro invocations
where the trailing comma is *known* to be syntactically optional.

## Statement

For an eligible macro invocation whose top-level arguments are
comma-separated:

- **Multi-line invocation** (the opening and closing delimiters
  are on different source lines): the last argument must be
  followed by a trailing comma.
- **Single-line invocation** (delimiters on the same source
  line): the last argument must *not* be followed by a trailing
  comma.

"Eligible" means the trailing comma is provably optional — see
the next section.

## Two eligibility modes

The hard part is deciding when adding or removing the trailing
comma is safe. A declarative macro's matchers can require a
trailing comma, forbid one, or accept either; a procedural
macro's grammar is opaque. Two complementary mechanisms decide
eligibility, named for *how* they identify an eligible macro.

### Name-based — curated macro list

A hard-coded list of macros known to accept the trailing comma
optionally. **Inclusion criterion:** the macro's matcher accepts
a top-level comma-separated argument list with a syntactically
optional trailing comma. The list may have any length — a
single-argument invocation like `dbg!(x)` or `env!("VAR")`
qualifies just as much as a multi-argument one. Multi-line
single-argument invocations get a trailing comma per rustfmt's
policy (`vec![\n    x\n]` → `vec![\n    x,\n]`); single-line
single-argument invocations have nothing to fix (no trailing
comma is present to remove and the invocation is already
single-line short).

Macros that use a different separator (`;` for `thread_local!`,
`lazy_static!`), that take a single non-list argument the
matcher won't follow with an optional comma (`include_str!`,
`compile_error!`, `cfg!`), or that pass tokens through verbatim
so a trailing comma would change their output (`stringify!`,
`paste::paste!`) do **not** qualify; they're deliberately
absent from the list below.

The list covers two groups:

- **`core` / `std` macros** that take comma-separated arguments:
  `vec!`, `format!`, `format_args!`, `print!`, `println!`,
  `eprint!`, `eprintln!`, `write!`, `writeln!`, `panic!`,
  `unimplemented!`, `todo!`, `unreachable!`, `assert!`,
  `assert_eq!`, `assert_ne!`, `debug_assert!`,
  `debug_assert_eq!`, `debug_assert_ne!`, `matches!`,
  `dbg!`, `concat!`, `env!`, `option_env!`.
- **Well-known third-party macros** with the same convention:
  `pretty_assertions::{assert_eq, assert_ne, assert_str_eq}`,
  `maplit::{hashmap, btreemap, hashset, btreeset, convert_args}`,
  `log::{log, error, warn, info, debug, trace}`,
  `tracing::{event, error, warn, info, debug, trace, span}`,
  `anyhow::{anyhow, bail, ensure}`.

  The list is curated, not exhaustive — projects extend it via
  configuration when they import a new such macro.

Name-based matching applies to any function-like macro
invocation, declarative or procedural: anything that the AST
represents as an `ast::MacCall`. **Attribute-style invocations
are out of scope** for this rule — `#[derive(...)]`,
`#[display(...)]`, `#[error(...)]`, `#[serde(...)]`, and the
rest live on `ast::Attribute` nodes that the lint's
`check_mac` callback does not visit. A separate
`attribute-trailing-comma` rule could handle those in a
follow-up (the comma-policy reasoning is the same, but the
visit path, configuration shape, and span layout all differ);
this rule restricts itself to `MacCall` to keep the
implementation single-purpose.

**Caveat — "comma-separated" means the rustfmt shape.** The
curated list and `extra_name_based` should only include macros
where commas are *required between items* and *optional only at
the trailing position* — the same policy rustfmt applies to
function calls and struct literals. Macros that treat commas as
*fully optional* separators throughout — `build-fs-tree`'s
`dir!` is one example, where each entry may or may not be
followed by a comma independently — must not be added. Forcing a
trailing comma on the last entry would clash with the no-comma
style users of those macros often choose. If such a macro slips
into the list, add it to `ignore` to opt it out.

### Matcher-based — declarative-macro auto-detection

For a `macro_rules!` macro whose definition is visible to the
compiler (the local crate, or a dependency whose macro body
rustc still has on hand), inspect the matcher arms:

- A macro arm whose final matcher position is `$(,)?` or
  `$(,)*` — used purely to absorb a trailing comma — can
  match the invocation with or without the comma. The
  optional-comma capture cannot be expanded into the body (a
  literal token in a matcher position cannot be referenced by
  `$name` in the expansion), so the trailing comma is purely
  syntactic. The lint may rewrite freely.
- An arm that ends in a literal `,` *or* in `$(,)+`
  *requires* at least one trailing comma; the lint must not
  remove it. `$(,)+` is the easy mis-read here — `+` matches
  one or more, so an invocation with no trailing comma at all
  would not have matched in the first place.
- An arm whose final position is something else (a non-comma
  token, or a `$name:tt` capture) doesn't carry a trailing
  comma at all and is out of scope.

The predicate is specifically about the *tail* of the top-level
matcher. A macro whose matcher makes every comma optional —
e.g., `$($key:literal => $value:expr $(,)?)*`, where each entry
may or may not be followed by a comma independently — has a
top-level tail that is the `)*` of the outer repetition, not a
top-level `$(,)?`. The predicate doesn't match, the lint
correctly skips, and users who write such macros without any
commas are not forced into a stray trailing one.

For a given invocation, the lint determines which arm matches
(or, conservatively, requires that *every* arm of the macro
accept both forms) and proceeds only if the matched arm is the
optional-comma kind.

Matcher-based matching does not extend to procedural macros:
the matcher is custom Rust code, not introspectable as a token
pattern.

### Why matcher-based is harder than name-based

Name-based matching is a name-set lookup on the resolved macro
`DefId`. The implementation is a `BTreeSet<&'static str>`
initialised at plugin start, plus the user's `extra_name_based`
paths.

Matcher-based matching has to:

1. Reach the `macro_rules!` matcher AST from the invocation's
   `DefId`. For local macros this is
   `tcx.hir_node_by_def_id(def_id)`, which returns a
   `Node<'tcx>` whose `ItemKind::MacroDef` carries the matcher
   arms. For dependency macros the matcher is reachable via the
   crate-metadata store (`tcx.cstore_untracked()` plus the
   macro-data query); see the implementation notes section for
   the exact call path. Some matchers are not re-exported
   across crate boundaries — those cases must degrade gracefully
   (treat as ineligible, do not warn).
2. Walk every arm's matcher token tree, locating an optional
   trailing comma at the tail of the top-level matcher —
   `$(,)?` or `$(,)*` per the predicate above.
3. Decide which arm matches the invocation — or refuse to
   touch the invocation if not every arm is the optional-comma
   kind, to stay safe in the face of ambiguity.

Name-based is a single afternoon. Matcher-based is a few days
plus the matcher-walking infrastructure, plus careful handling
of the multi-arm and cross-crate cases. Implement name-based
first; ship matcher-based in a follow-up.

## What to lint

For every macro invocation:

1. Resolve the macro `DefId`.
2. If the resolved path matches an entry in `ignore`, skip.
   `ignore` is checked first so a user opt-out wins over both
   name-based and matcher-based eligibility.
3. Decide eligibility:
   - If the path matches a name-based entry (built-in or
     user-configured via `extra_name_based`), eligible.
   - Otherwise, if matcher-based detection is enabled and the
     macro is a visible declarative macro whose matched arm
     ends in `$(,)?` or `$(,)*` (per the predicate above),
     eligible.
   - Otherwise, skip.
4. Inspect the *invocation token stream* — not the expansion —
   to confirm the call is shaped like a top-level
   comma-separated argument list. Skip if:
   - The body contains a top-level `;` (e.g., `vec![value;
     count]`, `thread_local! { static FOO: ...; static BAR:
     ...; }`). A top-level `;` indicates the macro uses `;` as
     its item separator, not commas. Note that step 3's
     eligibility check has already filtered macros to the
     curated comma-separated-list set, so this case mostly
     guards against the dual-arm `vec!` (`vec![el; n]` form)
     and any cross-arm matcher-based hits where one arm of the
     macro is comma-separated and another is `;`-separated.
   - The body is empty (only whitespace and comments between
     delimiters). Nothing to add or remove.
   - `=>` may appear at the top level between items — e.g.,
     `hashmap! { "a" => 1, "b" => 2 }` legitimately has a
     top-level `=>` per entry — and is fine. It is *not* a
     skip trigger.

   A zero-comma body is **not** a skip trigger. A single-item
   list still benefits from the multi-line trailing comma per
   rustfmt's policy (`vec![\n    x\n]` becomes
   `vec![\n    x,\n]`). The eligibility check at step 3 has
   already established that the macro is shaped like a
   comma-separated list, so a one-item invocation is a
   one-item list, not a token-tree passthrough.
5. Determine "single-line" vs "multi-line" by the source
   positions of the opening and closing delimiters. If they
   share a line, single-line; otherwise multi-line.
6. Locate the final top-level token before the closing
   delimiter:
   - **Multi-line, no trailing comma** → emit a diagnostic
     suggesting an inserted `,`.
   - **Single-line, trailing comma present** → emit a
     diagnostic suggesting removal of the `,`.
   - Otherwise, no diagnostic.

The autofix is `Applicability::MachineApplicable` for name-based
matches (curated list — the human vetted that the comma is
optional) and for matcher-based matches where every arm of the
macro accepts both forms. Matcher-based cases that picked one
matching arm out of several are `Applicability::MaybeIncorrect`
— the matched-arm analysis is conservative but a future macro
revision could shift which arm matches.

## Examples

### Name-based: `vec!`

```rust
// Bad: multi-line, missing trailing comma
let xs = vec![
    1,
    2,
    3
];

// Good
let xs = vec![
    1,
    2,
    3,
];
```

```rust
// Bad: single-line, gratuitous trailing comma
let xs = vec![1, 2, 3,];

// Good
let xs = vec![1, 2, 3];
```

```rust
// Bad: single-argument multi-line invocation, no trailing
// comma. Matches rustfmt's behaviour for function calls.
dbg!(
    expensive_function_call(arg)
);

// Good
dbg!(
    expensive_function_call(arg),
);
```

### Name-based: `assert_eq!`

```rust
// Bad: multi-line panic message
assert_eq!(
    actual,
    expected,
    "decoder mismatch: stream {stream_id} chunk {chunk_id}"
);

// Good
assert_eq!(
    actual,
    expected,
    "decoder mismatch: stream {stream_id} chunk {chunk_id}",
);
```

### Matcher-based: `macro_rules!` ending in `$(,)?`

```rust
macro_rules! comma_list {
    ($($item:expr),* $(,)?) => { /* ... */ };
}

// Bad: multi-line, missing trailing comma
comma_list!(
    a,
    b,
    c
);

// Good
comma_list!(
    a,
    b,
    c,
);
```

### Skipped: not shaped like a comma-separated list

```rust
// Skipped: `vec![value; count]` uses `;`, not a list separator.
// A top-level `;` indicates a different macro form.
let zeros = vec![0; 10];

// Skipped: `quote!` is a token-tree passthrough, not a
// comma-separated list. It isn't on the curated name-based
// list, and matcher-based detection's $(,)? / $(,)* predicate
// doesn't match its grammar — so step 3's eligibility check
// fails and the lint never reaches the trailing-comma check.
quote! {
    fn foo() -> i32 { 42 }
};
```

### Skipped: macro requires the trailing comma

```rust
macro_rules! always_comma {
    ($($item:expr,)*) => { /* ... */ };
}

// Skipped: removing the trailing comma here would fail to
// match. The matcher is `$($item:expr,)*` — every item must
// be followed by a literal comma, including the last.
always_comma!(
    a,
    b,
    c,
);
```

### Skipped: macro with fully-optional commas

```rust
// Matcher with per-item optional commas: every comma in the
// list is independently optional, so both comma-separated and
// no-comma styles are valid. `build-fs-tree::dir!` is a
// real-world example.
macro_rules! dir {
    ($($key:literal => $value:expr $(,)?)*) => { /* ... */ };
}

// Skipped: the matcher's top-level tail is `)*`, not a
// top-level `$(,)?`. Matcher-based detection's predicate
// (`$(,)?` at the tail of the top-level matcher) doesn't
// match, so the lint correctly leaves the call alone. The
// macro must also not be added to `extra_name_based` — users
// who write entries without any commas would otherwise get a
// stray trailing comma against an otherwise comma-free style.
dir! {
    "foo" => file!("a")
    "bar" => file!("b")
    "baz" => file!("c")
}
```

### Skipped: unknown procedural macro

```rust
// Skipped: `my_proc::custom!` is a procedural macro and is
// not in the user's `extra_name_based` list. The lint cannot
// inspect a proc-macro grammar.
my_proc::custom!(
    a,
    b,
    c
);
```

## Configuration

```toml
[macro_trailing_comma]
# Set to false to disable the rule entirely.
enabled = true

# Enable the matcher-based declarative-macro auto-detection.
# Defaults on. Disable to fall back to a pure name-based policy
# if matcher-based detection proves too noisy on a particular
# codebase.
matcher_based = true

# Additional macros to treat as name-based matches, beyond the
# built-in core/std and well-known third-party set. Each entry
# is a fully-qualified macro path (no trailing `!`) or a bare
# macro name to match by final segment only.
#
# Use this knob when:
# - A project depends on a third-party macro the built-in list
#   does not cover.
# - Matcher-based detection cannot see the macro definition
#   (cross-crate proc macro, or macro_rules! re-exported in a
#   way that loses the matcher).
# - The macro is a procedural one whose author guarantees the
#   trailing comma is optional.
extra_name_based = [
  # "my_crate::my_macro",
  # "another_macro",
]

# Macros for the lint to ignore, even when name-based or
# matcher-based detection would otherwise mark them eligible.
# Use this for macros where the project's own convention
# diverges (for example, macros whose body is more readable
# with the comma always present even on a single line). The
# name is `ignore` rather than `deny_list` because the lint
# never forbids the macro itself — it only declines to act on
# the invocation's trailing comma.
ignore = [
  # "my_crate::ascii_table",
]
```

## Implementation notes

- `EarlyLintPass::check_mac` over `ast::MacCall`. The early
  pass runs before macro expansion so the invocation's raw
  token stream is still on the AST node — the lint needs the
  source token tree, not the expansion result, to decide
  whether the closing delimiter is preceded by a comma.
- Macro path resolution: `MacCall::path` resolves to a `Res`
  via the resolver. From the resolved `DefId`, look the path
  up in the name-based list (built-in plus
  `extra_name_based`). For matcher-based detection, fetch the
  matcher arms from the resolved macro definition.
- Token-tree inspection: `MacCall::args` carries a
  `DelimArgs` whose `tokens: TokenStream` is the raw user
  input. Walk the top-level token stream, tracking nesting
  by `Delimiter` so commas inside nested groups are not
  mistaken for top-level separators.
- Single-line vs multi-line: compare
  `Span::lo()`/`Span::hi()` line numbers via the
  `SourceMap`. The opening and closing delimiter spans are
  available on the `DelimArgs` directly.
- Matcher walk: `macro_rules!` arms are
  `ast::MacroDef::body`'s LHS token streams. Walk the LHS,
  detect either of the optional-trailing-comma patterns
  (`$(,)?` or `$(,)*`) at the end of the top-level matcher —
  i.e., `OpenDelim(Paren) ... $(,)? CloseDelim(Paren)` or
  `OpenDelim(Paren) ... $(,)* CloseDelim(Paren)`. `$(,)+` and
  a bare literal `,` at the same position are *required*, not
  optional, and the walker must distinguish them (see the
  `take_optional_trailing_comma` note below).
  Matcher access depends on where the macro is defined: for a
  macro defined in the current crate, use
  `tcx.hir_node_by_def_id(def_id)` to reach the
  `ItemKind::MacroDef` AST. For a macro imported from a
  dependency, the matcher is in the dependency's rmeta and is
  reachable via `tcx.cstore_untracked()` plus the macro-data
  query. Unavailable matchers (some cross-crate cases) degrade
  to "ineligible".
- **Parser style.** The matcher walker is a
  parser-combinator-style `take_*` chain over the matcher
  token stream per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
  one combinator per matcher position kind (literal token,
  `$name:frag` capture, `$( ... )sep rep` repetition), with
  a top-level `take_optional_trailing_comma` that recognises
  `$(,)?` and `$(,)*` at the tail of an arm. `$(,)+` and a
  bare literal `,` are *not* optional-trailing-comma forms
  (both require at least one comma) and the combinator must
  classify them as "required" so the lint refuses to remove
  the comma.

### Difficulty

**Name-based: easy.** A name-set lookup keyed off a resolved
`DefId`, plus a token-stream scan for the trailing comma,
plus a span-based "is this multi-line" predicate. The
autofix is a one-character insertion or removal at a known
position — `MachineApplicable`.

**Matcher-based: hard.** The matcher walker has to handle
every matcher repetition shape, the multi-arm case (which
arm matched? do all arms agree on the optional comma?), and
the cross-crate "matcher not available" degradation. None of
this is conceptually deep; the work is in covering the
matcher grammar carefully and refusing to act when
ambiguous. Recommend landing name-based first as a
standalone PR, then layering matcher-based on top behind the
`matcher_based` configuration knob.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in
  this catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Severity

Warn.

## Interaction with sibling rules

- [`format-macro-wrap`](./format-macro-wrap.md) and
  [`print-macro-split`](./print-macro-split.md) reformat the
  *template literal* inside `format!` / `println!` / etc.
  When their `line_continuation` rewrite fires it produces a
  multi-line invocation; this rule then ensures that
  invocation carries a trailing comma. The two checks are
  complementary and converge to the same shape regardless of
  evaluation order.
- `rustfmt` itself handles every non-macro argument-list
  position. There is no overlap; this rule exists precisely
  because rustfmt opts out of macro bodies.
