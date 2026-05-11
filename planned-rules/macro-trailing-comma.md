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

## Two tiers of eligibility

The hard part is deciding when adding or removing the trailing
comma is safe. A declarative macro's matchers can require a
trailing comma, forbid one, or accept either; a procedural
macro's grammar is opaque. Two complementary mechanisms decide
eligibility:

### Tier 1 — well-known macro allow-list

A hard-coded list of macros known to accept the trailing comma
optionally. The list covers two groups:

- **`core` / `std` macros** that take comma-separated arguments:
  `vec!`, `format!`, `format_args!`, `print!`, `println!`,
  `eprint!`, `eprintln!`, `write!`, `writeln!`, `panic!`,
  `unimplemented!`, `todo!`, `unreachable!`, `assert!`,
  `assert_eq!`, `assert_ne!`, `debug_assert!`,
  `debug_assert_eq!`, `debug_assert_ne!`, `matches!`,
  `dbg!`, `concat!`, `env!`, `option_env!`, `stringify!`,
  `cfg!`, `compile_error!`, `include!`, `include_str!`,
  `include_bytes!`, `thread_local!`.
- **Well-known third-party macros** with the same convention:
  `pretty_assertions::{assert_eq, assert_ne, assert_str_eq}`,
  `maplit::{hashmap, btreemap, hashset, btreeset, convert_args}`,
  `serde_json::json`, `lazy_static::lazy_static`,
  `paste::paste`, `derive_more`-style derivable attributes
  whose argument list is comma-separated, `log::{log, error,
  warn, info, debug, trace}`, `tracing::{event, error, warn,
  info, debug, trace, span}`, `clap::{arg, command}`,
  `anyhow::{anyhow, bail, ensure}`, `thiserror`-style attribute
  argument lists.

  The list is curated, not exhaustive — projects extend it via
  configuration when they import a new such macro.

Tier 1 applies to *any* macro form: declarative, procedural
function-like, attribute-style derives that take a comma-
separated argument list. The matcher source is irrelevant
because the human author has vetted the entry.

### Tier 2 — declarative-macro auto-detection

For a `macro_rules!` macro whose definition is visible to the
compiler (the local crate, or a dependency whose macro body
rustc still has on hand), inspect the matcher arms:

- A macro arm whose final matcher position is `$(,)?` — or
  equivalently `$(,)*` / `$(,)+` used purely to absorb a
  trailing comma — can match the invocation with or without the
  comma. The optional-comma capture cannot be expanded into the
  body (a literal token in a matcher position cannot be
  referenced by `$name` in the expansion), so the trailing
  comma is purely syntactic. The lint may rewrite freely.
- An arm that ends in a literal `,` (no `?` / `*` / `+`)
  *requires* the trailing comma; the lint must not remove it.
- An arm whose final position is something else (a non-comma
  token, or a `$name:tt` capture) doesn't carry a trailing
  comma at all and is out of scope.

For a given invocation, the lint determines which arm matches
(or, conservatively, requires that *every* arm of the macro
accept both forms) and proceeds only if the matched arm is the
optional-comma kind.

Tier 2 does not extend to procedural macros: the matcher is
custom Rust code, not introspectable as a token pattern.

### Why Tier 2 is harder than Tier 1

Tier 1 is a name-set lookup on the resolved macro `DefId`. The
implementation is a `BTreeSet<&'static str>` initialised at
plugin start, plus the user's `extra_tier_1` paths.

Tier 2 has to:

1. Reach the `macro_rules!` matcher AST from the invocation's
   `DefId`. For local macros this is `tcx.hir().get_by_def_id`;
   for dependency macros the matcher is reachable via
   `tcx.crate_def_map(cnum)` and the macro's expansion data,
   which lives in the crate metadata. Some matchers are not
   re-exported across crate boundaries — those Tier-2 cases
   must degrade gracefully (treat as ineligible, do not warn).
2. Walk every arm's matcher token tree, locating the trailing
   `$(,)?` (or equivalent) per the predicate above.
3. Decide which arm matches the invocation — or refuse to
   touch the invocation if not every arm is the optional-comma
   kind, to stay safe in the face of ambiguity.

Tier 1 is a single afternoon. Tier 2 is a few days plus the
matcher-walking infrastructure, plus careful handling of the
multi-arm and cross-crate cases. The user's intuition is
correct: implement Tier 1 first; ship Tier 2 in a follow-up.

## What to lint

For every macro invocation:

1. Resolve the macro `DefId`.
2. Decide eligibility:
   - If the path matches a Tier-1 entry (built-in or
     user-configured), eligible.
   - Otherwise, if Tier 2 is enabled and the macro is a
     visible declarative macro whose matched arm ends in
     `$(,)?` (per the predicate above), eligible.
   - Otherwise, skip.
3. Inspect the *invocation token stream* — not the expansion —
   to confirm that the top-level argument list is purely comma-
   separated. Skip if:
   - The delimiter is opened but the body contains a top-level
     `;` (e.g., `vec![value; count]`), `=>` (token-tree macros
     like `quote!` arms), or any other top-level separator that
     isn't a comma.
   - There are zero arguments (no comma to add or remove).
4. Determine "single-line" vs "multi-line" by the source
   positions of the opening and closing delimiters. If they
   share a line, single-line; otherwise multi-line.
5. Locate the final top-level token before the closing
   delimiter:
   - **Multi-line, no trailing comma** → emit a diagnostic
     suggesting an inserted `,`.
   - **Single-line, trailing comma present** → emit a
     diagnostic suggesting removal of the `,`.
   - Otherwise, no diagnostic.

The autofix is `Applicability::MachineApplicable` for Tier 1
(curated allow-list — the human vetted that the comma is
optional) and for Tier 2 cases where every arm of the macro
accepts both forms. Tier 2 cases that picked one matching arm
out of several are `Applicability::MaybeIncorrect` — the
matched-arm analysis is conservative but a future macro
revision could shift which arm matches.

## Examples

### Tier 1: `vec!`

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

### Tier 1: `assert_eq!`

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

### Tier 2: locally-defined `macro_rules!`

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

### Skipped: non-comma top-level separator

```rust
// Skipped: vec![value; count] uses `;`, not a list separator
let zeros = vec![0; 10];

// Skipped: quote! arm uses `=>` and arbitrary tokens
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

### Skipped: unknown procedural macro

```rust
// Skipped: `my_proc::custom!` is a procedural macro and is
// not in the user's `extra_tier_1` list. The lint cannot
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

# Enable the Tier 2 declarative-macro auto-detection. Defaults
# on. Disable to fall back to a pure allow-list policy if Tier 2
# proves too noisy on a particular codebase.
tier_2 = true

# Additional macros to treat as Tier 1, beyond the built-in
# core/std and well-known third-party set. Each entry is a
# fully-qualified macro path (no trailing `!`) or a bare macro
# name to match by final segment only.
#
# Use this knob when:
# - A project depends on a third-party macro the built-in list
#   does not cover.
# - Tier 2 cannot see the macro definition (cross-crate proc
#   macro, or macro_rules! re-exported in a way that loses the
#   matcher).
# - The macro is a procedural one whose author guarantees the
#   trailing comma is optional.
extra_tier_1 = [
  # "my_crate::my_macro",
  # "another_macro",
]

# Macros to never lint, even if they match Tier 1 or Tier 2.
# Use this for macros where the project's own convention
# diverges (for example, macros whose body is more readable with
# the comma always present even on a single line).
deny_list = [
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
  up in the Tier-1 allow-list (built-in plus
  `extra_tier_1`). For Tier 2, fetch the matcher arms from
  the resolved macro definition.
- Token-tree inspection: `MacCall::args` carries a
  `DelimArgs` whose `tokens: TokenStream` is the raw user
  input. Walk the top-level token stream, tracking nesting
  by `Delimiter` so commas inside nested groups are not
  mistaken for top-level separators.
- Single-line vs multi-line: compare
  `Span::lo()`/`Span::hi()` line numbers via the
  `SourceMap`. The opening and closing delimiter spans are
  available on the `DelimArgs` directly.
- Tier 2 matcher walk: `macro_rules!` arms are
  `ast::MacroDef::body`'s LHS token streams. Walk the LHS,
  detect the `$(,)?` pattern at the end of the top-level
  matcher (`OpenDelim(Paren) ... $(,)? CloseDelim(Paren)`).
  For dependency macros, the matcher comes from
  `tcx.hir_node_by_def_id(...)` if local, or via the macro
  metadata in `tcx.cstore_untracked()` for external macros;
  unavailable matchers degrade to "ineligible".
- **Parser style.** The matcher walker for Tier 2 is a
  parser-combinator-style `take_*` chain over the matcher
  token stream per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
  one combinator per matcher position kind (literal token,
  `$name:frag` capture, `$( ... )sep rep` repetition), with
  a top-level `take_optional_trailing_comma` that recognises
  `$(,)?`, `$(,)*`, `$(,)+` at the tail of an arm.

### Difficulty

**Tier 1: easy.** A name-set lookup keyed off a resolved
`DefId`, plus a token-stream scan for the trailing comma,
plus a span-based "is this multi-line" predicate. The
autofix is a one-character insertion or removal at a known
position — `MachineApplicable`.

**Tier 2: hard.** The matcher walker has to handle every
matcher repetition shape, the multi-arm case (which arm
matched? do all arms agree on the optional comma?), and the
cross-crate "matcher not available" degradation. None of
this is conceptually deep; the work is in covering the
matcher grammar carefully and refusing to act when
ambiguous. Recommend landing Tier 1 first as a standalone
PR, then layering Tier 2 on top behind the `tier_2`
configuration knob.

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
