# `print_macro_split`

**Source:** project convention.

## Statement

A `println!`-style macro call whose format template contains
embedded newlines *and* whose source line exceeds the configured
width is hard to read and harder to scan in a diff:

```rust
println!("error: The error was caused by {err_src}\nhint: Run {magic_cmd} to solve the problem");
```

Two readable alternatives are available, configurable per project:

- **`multiple_calls`** (default) — split the template at every
  embedded `\n` and emit one macro call per resulting line. Each
  call reads as a single statement and the source columns shrink
  proportionally:
  ```rust
  println!("error: The error was caused by {err_src}");
  println!("hint: Run {magic_cmd} to solve the problem");
  ```

- **`line_continuation`** — keep the call as one statement but
  fold the long template across multiple source lines with the
  backslash-newline escape:
  ```rust
  println!(
      "error: The error was caused by {err_src}\n\
      hint: Run {magic_cmd} to solve the problem",
  );
  ```

Both rewrites produce the same byte-for-byte output as the
original call. The choice between them is purely aesthetic.

## Why not `format!`-family

The rule does **not** apply to `format!`, `format_args!`, or any
other macro that *returns a value* (or that terminates the
program). Splitting such calls would change the type of the
expression, lose the result, or never reach the second call.
Specifically:

- `format!` / `format_args!` — produce `String` /
  `fmt::Arguments`. Can't be split into two values silently.
- `panic!` / `unimplemented!` / `todo!` / `unreachable!` —
  terminate; the second call never runs.
- `assert!` / `assert_eq!` / `assert_ne!` and the
  `debug_assert*` family — the trailing string is a one-shot
  failure message; splitting changes panic semantics.

The rule only fires on macros that are pure side-effect
producers and whose output is byte-equivalent under splitting.

## What to lint

For every invocation of a target macro:

1. Resolve the macro path. Skip if it isn't in `target_macros`.
2. Locate the format template (the first string-literal
   positional argument, conventionally; for `write!`/`writeln!`
   it's the second argument because the first is the writer).
3. Skip if the template is not a string literal (it's a runtime
   expression, e.g., a constant or `concat!` result).
4. Skip if the template contains no `\n` (no split is possible).
5. Compute the source-line span of the entire macro invocation.
   If its width is `≤ max_line_width`, skip — the call is short
   enough to leave alone.
6. Apply the configured `style`:
   - `multiple_calls`: split the template at each interior `\n`
     and emit one macro call per resulting segment. Trailing
     `\n` on the last segment is dropped because the macro
     itself ("ln" suffix) supplies one. For non-`ln` macros
     (`print!`, `eprint!`, `write!`), the trailing `\n` is
     preserved on the last call.
   - `line_continuation`: replace each interior `\n` with
     `\n\<newline><indent>` where `<indent>` matches the source
     indentation of the original literal. Wrap the macro
     invocation across multiple lines if needed.
   - `preserve`: emit nothing.

## Examples

### Default style (`multiple_calls`)

```rust
// Bad: long source line, embedded newline, splittable macro
println!("error: The error was caused by {err_src}\nhint: Run {magic_cmd} to solve the problem");

// Good
println!("error: The error was caused by {err_src}");
println!("hint: Run {magic_cmd} to solve the problem");
```

```rust
// Bad: writeln! with two segments
writeln!(f, "header: {h}\nbody: {b}\nfooter: {ft}")?;

// Good
writeln!(f, "header: {h}")?;
writeln!(f, "body: {b}")?;
writeln!(f, "footer: {ft}")?;
```

### Style `line_continuation`

```rust
// Bad
println!("error: The error was caused by {err_src}\nhint: Run {magic_cmd} to solve the problem");

// Good
println!(
    "error: The error was caused by {err_src}\n\
    hint: Run {magic_cmd} to solve the problem",
);
```

### Skipped contexts

```rust
// Skipped: format! returns a value
let s = format!("a\nb\nc");

// Skipped: panic! terminates
panic!("a\nb");

// Skipped: assert! message is one-shot
assert!(cond, "a\nb");

// Skipped: short source line
println!("a\nb");
```

## Configuration

```toml
[print_macro_split]
style = "multiple_calls"   # or "line_continuation" or "preserve"

# Source-line width that triggers the rule. Default 100 matches
# rustfmt's column default. Common alternatives are 80 (terminal)
# or 120 (modern wide editors). Width is the unicode display
# width of the line containing the macro invocation, not its
# byte length.
max_line_width = 100

# Macros eligible for splitting. The defaults cover every macro
# whose output is byte-equivalent under multi-call splitting.
# Macros that return a value or terminate are deliberately absent.
target_macros = [
  # stdout/stderr writers
  "println", "eprintln", "print", "eprint",
  # `Write` writers
  "writeln", "write",
  # log family
  "log", "error", "warn", "info", "debug", "trace",
]
```

## Implementation notes

- `LateLintPass::check_expr` on `ExprKind::Call` and macro
  invocations. For macros, dispatch via `Span::from_expansion()`
  and the expansion's macro `DefId`; reuse
  `clippy_utils::macros::FormatArgsExpn` to walk the format-args
  arguments uniformly across the supported macros.
- Template extraction:
  - `println!` / `eprintln!` / `print!` / `eprint!`: first arg.
  - `write!` / `writeln!`: second arg.
  - `log::*!`: first arg (or second, after a target). Use
    clippy_utils' helper to find it.
- Source-line width: take the macro invocation's full `Span`,
  resolve to the source map, compute the unicode display width
  via the `unicode-width` crate. Same dependency and approach as
  [`prefer-text-block`](./prefer-text-block.md); share the helper.
- **Parser style.** Implement the format-template scanner as
  parser-combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
  reuse the placeholder/literal helpers from
  [`derive-more-inlined-args`](./derive-more-inlined-args.md) and
  the escape scanner in `src/rules/prefer_raw_string.rs`. The
  split is at decoded-`\n` boundaries; placeholders that straddle a `\n` are
  impossible because `\n` cannot appear *inside* a `{...}`
  placeholder, so the split is always between placeholders.

### Splitting positional vs named args

The `multiple_calls` autofix has to assign each macro call only
the arguments its template fragment uses:

- **All inlined / named args** (`println!("a {x}\nb {y}", x = X, y = Y)`):
  trivially split. Each new call references only the names that
  appear in its fragment. The argument list of each call is the
  subset of `(x, y, ...)` that the fragment uses.
- **All positional `{}` args** (`println!("a {} b\nc {} d", X, Y)`):
  re-index the placeholders per fragment. The first fragment
  takes `X`, the second takes `Y`. The lint must walk the format
  string to count placeholders per fragment and slice the
  argument list accordingly.
- **All positional `{N}` args** (`println!("a {0}\nb {1}", X, Y)`):
  the indices stay valid only if every fragment carries the full
  argument list — which would defeat the readability win. The
  autofix renumbers each fragment's `{N}` references to start at
  `0` and slices the argument list to match.
- **Mixed positional and named**: the autofix bails to a
  help-only suggestion. The combinatorial cases aren't worth the
  implementation complexity.

### Difficulty

**Medium for `line_continuation`, hard for `multiple_calls`.**

`line_continuation` is a pure syntactic rewrite of the template.
`MachineApplicable`.

`multiple_calls` requires understanding the format string deeply
enough to slice the argument list per fragment. The all-inlined
case is straightforward; the positional cases need careful
re-indexing. The mixed case bails. Suggested fix is
`MachineApplicable` only when *all* placeholders in the original
template are inlined / named (no positional `{}` or `{N}`);
`MaybeIncorrect` for purely-positional templates after
re-indexing; help-only for mixed templates.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Default state

Active by default.

## Interaction with sibling rules

- [`prefer-text-block`](./prefer-text-block.md) handles multi-line
  string literals in *non-template* positions (a `let s = "a\nb"`
  binding, a return value). It explicitly skips format templates,
  which is where this rule picks up.
- [`derive-more-inlined-args`](./derive-more-inlined-args.md)
  inlines positional arguments in `#[display(...)]` /
  `#[debug(...)]` attributes. Running it before
  `print_macro_split` increases the share of templates the
  `multiple_calls` autofix can apply cleanly (every additional
  inlined arg removes a positional re-indexing case).

The three rules together cover the three places multi-line text
appears in code: bare literals (`prefer-text-block`),
display/debug attributes (`derive-more-inlined-args`), and print
macros (this rule).
