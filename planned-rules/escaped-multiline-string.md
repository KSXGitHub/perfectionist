# `escaped_multiline_string`

**Source:** project convention.

## Statement

A multi-line string literal that escapes its newlines as `\n`
inside one quoted expression is hard to read:

```rust
let banner = "foo\nbar\nbaz";
```

Two better shapes are available, depending on project preference:

- **`text_block_macros`** (default) — use the
  [`text-block-macros`](https://crates.io/crates/text-block-macros)
  crate. Each line is its own quoted token; the macro joins them
  with newlines:

  ```rust
  use text_block_macros::text_block;

  let banner = text_block! {
      "foo"
      "bar"
      "baz"
  };
  ```

- **`line_continuation`** — use Rust's `\<newline>` escape to
  break a single string literal across source lines without
  introducing extra whitespace:

  ```rust
  let banner = "foo\n\
                bar\n\
                baz";
  ```

Both produce the same string at runtime. The rule lets a project
enforce one consistently.

The rule does **not** fire on string literals that are *templates*
for a format-family macro or a derive_more / thiserror
display-style attribute — those literals are interpreted by the
macro and switching forms breaks the template.

## What to lint

For every string literal (`ExprKind::Lit` of `LitKind::Str`) that
satisfies *either* trigger:

- Its decoded value contains at least `min_newlines_to_trigger`
  newline characters (default 2), **or**
- Any single line in its decoded value (between newline boundaries,
  or the entire value when no newlines are present) has unicode
  display width strictly greater than `max_line_width` (default
  100). "Width" is computed by Unicode display width — wide CJK
  characters count as 2 cells, combining marks count as 0, ASCII
  counts as 1 — not by `char` or byte count.

then:

1. Check the literal's enclosing context. Skip if the literal is:
   - The first positional argument of a recognised format-family
     macro (`format!`, `println!`, `eprintln!`, `format_args!`,
     `write!`, `writeln!`, `print!`, `eprint!`, `panic!`,
     `unimplemented!`, `todo!`, `unreachable!`, `assert!`,
     `assert_eq!`, `assert_ne!`, the `debug_assert*` family, the
     `log::*!` family). The argument list is configurable.
   - Already inside a `text_block!` or `text_block_fnl!` macro
     invocation. Avoids recursion on already-fixed code.
   - Inside an attribute meta-item, full stop. That covers
     `#[display(...)]`, `#[debug(...)]`, `#[error(...)]`,
     `#[doc = "..."]`, and any other attribute whose argument
     happens to contain a multi-line string.
2. Apply the configured `style`. The handling depends on which
   trigger fired:
   - **Newline trigger** (the literal has `≥ min_newlines_to_trigger`
     newlines):
     - `text_block_macros`: split on `\n`. Determine whether the
       literal ends in a trailing newline. Suggest
       `text_block_fnl! { ... }` if it does (the `_fnl` variant
       adds the trailing newline that the join itself omits);
       otherwise suggest `text_block! { ... }`.
     - `line_continuation`: synthesise the multi-line literal form
       by replacing each interior `\n` with `\n\<newline><indent>`
       where `<indent>` matches the source column of the original
       literal. The result is a single literal whose decoded value
       is identical.
   - **Width trigger only** (the literal has no qualifying newlines
     but at least one line exceeds `max_line_width`): always
     suggest `line_continuation`, regardless of the configured
     `style`. `text_block_macros` would *insert* newlines that
     weren't there, changing the runtime value; the line-
     continuation form is the only rewrite that preserves the
     value while breaking the source line. The lint splits at the
     last whitespace boundary that fits the budget, with a
     fallback to a hard split at the budget boundary if no
     whitespace is available.
   - **Both triggers** (multi-line *and* an over-width line): apply
     the configured `style` for the multi-line shape, then run the
     width check on each resulting per-line literal and apply
     line-continuation splits as needed. A `text_block!` invocation
     can carry its own `\<newline>` continuations inside any of
     its quoted lines.

## Examples

### Default style (`text_block_macros`)

**Avoid:** two or more newlines, not a template

```rust
let banner = "foo\nbar\nbaz";
```

**Prefer:** (no trailing newline)

```rust
let banner = text_block! {
    "foo"
    "bar"
    "baz"
};
```

**Avoid:** trailing newline

```rust
let banner = "foo\nbar\nbaz\n";
```

**Prefer:**

```rust
let banner = text_block_fnl! {
    "foo"
    "bar"
    "baz"
};
```

### Style `line_continuation`

**Avoid:**

```rust
let banner = "foo\nbar\nbaz";
```

**Prefer:**

```rust
let banner = "foo\n\
              bar\n\
              baz";
```

### Width trigger (single long line)

**Avoid:** one line that exceeds `max_line_width = 100`

```rust
let url = "https://very-long-subdomain.example.com/api/v2/resources/very-long-identifier?param=value";
```

**Prefer:** line_continuation form keeps the runtime value identical

```rust
let url = "https://very-long-subdomain.example.com/api/v2/resources/\
           very-long-identifier?param=value";
```

The lint never suggests `text_block_macros` for the width-only
case — splitting into multiple `text_block!` quoted lines would
insert `\n` characters that weren't in the original literal,
changing the runtime value.

### Both triggers (multi-line and a long line)

**Avoid:** two newlines AND the middle line is too long

```rust
let banner = "header\nthis is a very long line that exceeds the configured max_line_width\nfooter";
```

**Prefer:** (style = text_block_macros) outer text_block, inner line-continuation on the long quoted line

```rust
let banner = text_block! {
    "header"
    "this is a very long line that exceeds the \
     configured max_line_width"
    "footer"
};
```

### Skipped contexts

**Not flagged:**

```rust
println!("a\nb\nc\n{x}", x = 42);  // format template

// derive_more display template
#[derive(Display)]
#[display("line1\nline2\nline3")]
struct Banner;

// doc attribute
#[doc = "first line\nsecond line\nthird line"]
fn documented() {}

let _ = text_block! { "foo" "bar" "baz" };  // already inside text_block!
```

## Configuration

```toml
["perfectionist::escaped_multiline_string"]
style = "text_block_macros"  # or "line_continuation"

# Minimum number of `\n` characters in the decoded value before
# the rule fires. Default 2; raise to 3+ to ignore the most
# common two-line case if the project tolerates it. Set to 0 to
# disable the newline trigger entirely.
min_newlines_to_trigger = 2

# Maximum unicode display width of any single line in the literal.
# Lines longer than this fire the rule even when there are no
# embedded newlines. Default 100 matches rustfmt's column limit;
# common alternatives are 80 (terminal) or 120 (modern wide
# editors). Set to 0 to disable the width trigger entirely.
max_line_width = 100

# Format-family macros whose first positional argument is a
# template and should not be flagged.
format_macros = [
  "format", "println", "eprintln", "format_args",
  "write", "writeln", "print", "eprint",
  "panic", "unimplemented", "todo", "unreachable",
  "assert", "assert_eq", "assert_ne",
  "debug_assert", "debug_assert_eq", "debug_assert_ne",
  # `log::*!` family
  "error", "warn", "info", "debug", "trace", "log",
]

# text-block-macros invocations whose arguments are themselves
# the rewritten form; skip to avoid the lint firing on its own
# suggested output.
text_block_macros_paths = [
  "text_block_macros::text_block",
  "text_block_macros::text_block_fnl",
]

# When `style = "text_block_macros"`, the autofix suggests an
# import of the macro. If the project already re-exports
# `text_block!` from a different path (an internal prelude, etc.),
# set this to override which path the suggestion uses.
text_block_import_path = "text_block_macros::text_block"
text_block_fnl_import_path = "text_block_macros::text_block_fnl"
```

## Implementation notes

- `LateLintPass::check_expr` on `ExprKind::Lit` of `LitKind::Str`.
  Use the literal's *decoded* value (`lit.symbol_unescaped`) for
  the newline count; the source spelling can use `\n` escapes,
  literal `\<newline>` continuations, raw form, or any combination.
- Context detection:
  - **Format-macro skip.** Check `Span::from_expansion()` on the
    literal. If the outer expansion's macro path matches one of
    `format_macros` *and* the literal is the first
    positional argument, skip. Use
    `clippy_utils::macros::FormatArgsExpn` (or the
    diagnostic-name match) to walk the format-macro arguments
    cleanly.
  - **text_block skip.** Same `Span::from_expansion()` check
    against `text_block_macros_paths`. The literal sits inside
    the macro's token stream; once the expansion is identified,
    skip without further inspection.
  - **Attribute skip.** Walk the literal's HIR ancestors with
    `tcx.hir_parents(...)`. If any ancestor is an attribute meta
    list (`AttrKind::Normal`), skip. This catches `#[display]`,
    `#[doc]`, and every other attribute uniformly.
- Newline counting: walk `lit.symbol_unescaped` and count
  `\n` codepoints. The threshold is the count, not the line
  count — a string with one trailing newline counts as one,
  not as two lines.
- Width measurement: pull in the
  [`unicode-width`](https://crates.io/crates/unicode-width)
  crate (a single small dep) and use
  `UnicodeWidthStr::width(line)` on each `\n`-delimited segment.
  Don't count code points (wrong for CJK / emoji), don't count
  bytes (wrong for any non-ASCII). The threshold is *strict
  greater than*: `width == max_line_width` is fine,
  `width == max_line_width + 1` fires.
- Width-trigger autofix split point: scan the offending line
  right-to-left from the budget boundary for the last whitespace
  character (`' '`, `'\t'`); split there. If no whitespace is
  available within the budget, hard-split at the boundary —
  `\<newline>` is valid in any byte position outside escape
  sequences. The lint emits one continuation per line that
  exceeds the budget; a paragraph split into N continuations is
  one diagnostic with one suggestion containing all N splits.
- Trailing-newline detection (for the `text_block_macros` ↔
  `text_block_fnl` choice): the decoded value's last character
  is `\n` ⇒ use `_fnl`.
- **Parser style.** The literal-decoder side is provided by
  rustc; no parser-combinator work needed for the splitting.
  The `line_continuation` synthesizer's indentation calculation
  uses the source map's `lookup_char_pos` to find the column of
  the original literal, then pads each continuation line to
  match. Implement that pad calculation as a small helper rather
  than a regex.

### Difficulty

**Medium.** The literal walk is trivial; the context detection
(distinguishing template positions from non-template positions)
is the bulk of the work. The format-macro skip can lean on
clippy_utils' `FormatArgsExpn` helper for the well-known cases;
attribute skipping is a one-line ancestor walk.

The autofix:

- `text_block_macros` style: assemble the new `text_block!` /
  `text_block_fnl!` invocation as a string substitution. Mark
  `Applicability::MaybeIncorrect` because the rewrite assumes
  the project depends on `text-block-macros` and (when no
  existing import is in scope) needs the suggested `use`
  statement to be added separately.
- `line_continuation` style: pure syntactic rewrite of the
  literal's source spelling. `Applicability::MachineApplicable`.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Default state

Active by default. The default `style = "text_block_macros"`
reflects the catalogue's preferred form; projects that don't
want the external-crate dependency switch to `line_continuation`.

## Interaction with `perfectionist::avoidable_string_escapes`

The two rules look at the same expression type but at different
properties:

- `perfectionist::avoidable_string_escapes` (when applied) rewrites a literal's *quote
  delimiters* to avoid escapes for printable characters.
- `escaped-multiline-string` (when applied) rewrites a literal's
  *shape* to spread newlines across multiple source lines.

A multi-line string with both `\"` escapes and `\n` separators
hits both lints. The natural application order is
`escaped-multiline-string` first (lift the lines into a `text_block!`
or continued literal), then `perfectionist::avoidable_string_escapes` on each
resulting per-line literal. The two lints converge on a final
form like:

```rust
text_block! {
    r#"first {"json": "line"}"#
    r#"second {"more": "json"}"#
}
```

Both rules are independent and neither suppresses the other.

## Interaction with `perfectionist::dedented_multiline_string`

The two rules do different things on different source spellings and
stay fully independent — neither defers to or suppresses the other.
This rule targets newlines compressed onto one (or few) source lines
as `\n` escapes;
[`dedented-multiline-string`](./dedented-multiline-string.md)
targets newlines spelled as *raw* source breaks that drop the
literal's body out of the surrounding indentation. Because this
rule's motivating shape is the `\n`-escaped form and the sibling's
is the raw-break form, they rarely fire on the same literal in
practice; keeping each trigger self-contained (no cross-rule
avoidance) keeps both implementations simple.
