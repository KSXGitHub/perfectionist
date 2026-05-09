# `format_macro_wrap`

**Source:** project convention. Sibling to
[`print-macro-split`](./print-macro-split.md), which covers the
splittable side-effect macros. This rule handles the *un*splittable
ones — `format!`, `panic!`, the assert family, etc. — which can't
be turned into multiple calls without changing semantics.

## Statement

A `format!`-style macro call whose source line exceeds the
configured width is hard to read and produces a noisy diff:

```rust
format!("error: The error was caused by {err_src}\nhint: Run {magic_cmd} to solve the problem")
```

Unlike `println!`, splitting `format!` into multiple calls would
change the result type (one `String` becomes two). The only
applicable rewrite is folding the template across multiple source
lines with `\<newline>` continuations:

```rust
format!(
    "error: The error was caused by {err_src}\n\
    hint: Run {magic_cmd} to solve the problem",
)
```

The rule has only one suggested form (in contrast to
[`print-macro-split`](./print-macro-split.md), which offers two)
because `multiple_calls` is not viable for these macros.

## What to lint

For every invocation of a target macro:

1. Resolve the macro path. Skip if it isn't in `target_macros`.
2. Locate the format template inside the argument list:
   - `format!` / `format_args!`: first arg.
   - `panic!` / `unimplemented!` / `todo!` / `unreachable!`:
     first arg.
   - `assert!`: third arg (`assert!(cond, "msg ...")`); skip if
     no message is present.
   - `assert_eq!` / `assert_ne!`: third arg
     (`assert_eq!(a, b, "msg ...")`); skip if no message.
   - `debug_assert*` family: same shape as the corresponding
     non-debug macro.
3. Skip if the template is not a string literal (it's a runtime
   expression, e.g., a constant or `concat!` result).
4. Compute the source-line span of the entire macro invocation.
   If its width is `≤ max_line_width`, skip — the call is short
   enough to leave alone. Width is unicode display width, the
   same metric as
   [`prefer-text-block`](./prefer-text-block.md) and
   [`print-macro-split`](./print-macro-split.md).
5. Emit a diagnostic suggesting the line-continuation rewrite:
   - Replace each interior `\n` in the template with
     `\n\<newline><indent>`.
   - For long stretches without `\n`, also break at the last
     whitespace within the budget, with a hard split at the
     boundary as fallback.
   - `<indent>` matches the source column of the template
     literal.
   - Wrap the macro invocation across multiple lines if the
     resulting template doesn't fit on one continuation line —
     i.e., move the template literal onto its own line and the
     closing `)` onto a separate trailing line, matching the
     example above.

## Examples

```rust
// Bad: long source line; format! can't be split into multiple calls
format!("error: The error was caused by {err_src}\nhint: Run {magic_cmd} to solve the problem")

// Good
format!(
    "error: The error was caused by {err_src}\n\
    hint: Run {magic_cmd} to solve the problem",
)
```

```rust
// Bad: panic! with a long message
panic!("invariant violated: expected {expected} but got {actual} after {steps} iterations");

// Good
panic!(
    "invariant violated: expected {expected} but got {actual} \
    after {steps} iterations",
);
```

```rust
// Bad: assert_eq! message is too long
assert_eq!(actual, expected, "decoder mismatch: stream {stream_id} chunk {chunk_id} produced wrong output");

// Good
assert_eq!(
    actual,
    expected,
    "decoder mismatch: stream {stream_id} chunk {chunk_id} \
    produced wrong output",
);
```

```rust
// Skipped: short source line, even with embedded newline
format!("a\nb")

// Skipped: not in target_macros (println! is splittable; covered by print-macro-split)
println!("a\nb {x}")

// Skipped: not in target_macros (write! is splittable)
write!(f, "a\nb {x}")?;
```

## Configuration

```toml
[format_macro_wrap]
# Set to false to disable the rule entirely.
enabled = true

# Source-line width that triggers the rule. Default 100 matches
# rustfmt's column default. Width is unicode display width of the
# line containing the macro invocation, not its byte length.
max_line_width = 100

# Macros eligible for line-continuation wrapping. The defaults
# cover every macro that produces a value or terminates the
# program — the ones `print-macro-split` does *not* cover.
target_macros = [
  # value-producing
  "format", "format_args",
  # terminating
  "panic", "unimplemented", "todo", "unreachable",
  # one-shot diagnostic
  "assert", "assert_eq", "assert_ne",
  "debug_assert", "debug_assert_eq", "debug_assert_ne",
]
```

## Implementation notes

- `LateLintPass::check_expr` on macro invocations. Dispatch via
  `Span::from_expansion()` and the expansion's macro `DefId`.
  Reuse `clippy_utils::macros::FormatArgsExpn` to find the
  format template across the supported macros uniformly — it
  handles the "is this `assert!` with a message?" cases for
  free.
- Source-line width: take the macro invocation's full `Span`,
  resolve to the source map, compute the unicode display width
  via the `unicode-width` crate. Same dependency and helper as
  [`prefer-text-block`](./prefer-text-block.md) and
  [`print-macro-split`](./print-macro-split.md).
- **Parser style.** Implement the template scanner as parser-
  combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md).
  Reuse the placeholder/literal helpers from
  [`derive-more-inlined-args`](./derive-more-inlined-args.md),
  [`prefer-raw-string`](./prefer-raw-string.md), and
  [`print-macro-split`](./print-macro-split.md). The split logic
  is the same as `prefer-text-block`'s width-trigger split:
  scan for the last whitespace within the budget, hard-split at
  the boundary as fallback.

### Difficulty

**Medium.** The rewrite is a pure syntactic reformat of the
template literal — no argument re-slicing, no
multiple-call generation, no semantic equivalence proof needed.
The mechanical complexity is in choosing where to break long
runs of non-whitespace text without losing the indentation
context for the resulting source.

Autofix is `MachineApplicable` because the rewrite produces a
template byte-equivalent to the original; the macro invocation's
behaviour is unchanged.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Severity

Warn.

## Interaction with sibling rules

Together, three rules cover every place a multi-line or
otherwise-too-long template appears in source:

- [`prefer-text-block`](./prefer-text-block.md) — bare string
  literals (`let s = "a\nb"`, return values).
- [`print-macro-split`](./print-macro-split.md) — splittable
  side-effect macros (`println!`, `writeln!`, `log::*!`).
  Two-style choice between multi-call splitting and line
  continuation.
- `format_macro_wrap` (this rule) — value-producing or
  terminating macros (`format!`, `panic!`, `assert!`). Only
  line continuation is viable.

Each rule's `target_macros` (or skip-list) is disjoint from the
others by design; a given macro invocation is the responsibility
of exactly one of the three.
