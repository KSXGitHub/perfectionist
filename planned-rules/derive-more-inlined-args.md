# `derive_more_inlined_args`

**Source:** project convention. Clippy's `clippy::uninlined_format_args`
catches the `format!("... {} ...", name)` → `format!("... {name} ...")`
rewrite for the standard formatting macros, but it does not extend
to `#[display("...", arg, ...)]` attributes from `derive_more`. This
rule fills that gap.

## Statement

In a `#[display("format string", arg, arg, ...)]` (or any other
`derive_more` attribute that accepts `format!`-shaped arguments — see
*Scope* below), prefer the *inlined* form when an argument is a
simple identifier.

**Avoid:**

```rust
#[derive(Display)]
#[display("({}, {})", x, y)]
struct Point { x: i32, y: i32 }
```

**Prefer:**

```rust
#[derive(Display)]
#[display("({x}, {y})")]
struct Point { x: i32, y: i32 }
```

The rewrite mirrors `clippy::uninlined_format_args` for `format!`:
identifier-only arguments (including derive_more's positional
`_0`, `_1`, … placeholders) inline cleanly; expressions that are
not bare identifiers are left alone because Rust's format-args
capture syntax does not accept paths or method calls.

## Scope

The lint covers every derive_more attribute whose argument list
follows `format!` shape:

- `#[display("...", args)]` (from `derive_more::Display`).
- `#[debug("...", args)]` if the project uses `derive_more`'s
  `Debug` derive with a custom format.
- Custom-format error attributes that the project defines on top
  of `derive_more`. The recognised attribute paths are configurable.

Attributes whose arguments are not format-string shaped
(`#[from(forward)]`, `#[error(ignore)]`, `#[deref]`, etc.) are
ignored.

## What to lint

For every recognised attribute on a struct, enum, or variant:

1. Parse the attribute's argument list. The first argument must be
   a string literal; subsequent arguments are expressions.
2. Walk the format string, locating `{}` and `{N}` placeholders.
3. Pair each placeholder with its corresponding argument by
   position. Skip placeholders that already use named-capture
   syntax (`{x}`).
4. Classify the argument:
   - **Simple identifier** (`x`, `_0`, `field_name`): inlinable.
     Suggest replacing the placeholder with `{ident}` and removing
     the argument from the trailing list.
   - **Anything else** (`self.field`, `self.method()`, `compute(x)`,
     literals, casts): not inlinable. Leave the placeholder alone.
5. If at least one argument was inlined, emit a single diagnostic
   per attribute with a `MachineApplicable` suggestion containing
   the rewritten attribute.

The lint never flags a placeholder that uses a non-default format
spec when the inlined form would change rendering — `{:#x}` /
`{:>5}` / `{:.3}` etc. all carry over verbatim, so the inlined
suggestion preserves the spec: `{:>5}` paired with `name` becomes
`{name:>5}`.

## Examples

### Simple-ident args

**Avoid:**

```rust
#[display("({}, {})", x, y)]
```

**Prefer:**

```rust
#[display("({x}, {y})")]
```

### Positional placeholders with positional args

**Avoid:**

```rust
#[display("({}, {})", _0, _1)]
```

**Prefer:**

```rust
#[display("({_0}, {_1})")]
```

### Mixed: only the inlinable arg is rewritten

**Avoid:**

```rust
#[display("{} (line {})", file, self.line_number())]
```

**Prefer:**

```rust
#[display("{file} (line {})", self.line_number())]
```

### Format spec preserved

**Avoid:**

```rust
#[display("[{:>5}]", code)]
```

**Prefer:**

```rust
#[display("[{code:>5}]")]
```

## Configuration

```toml
[derive_more_inlined_args]
# Attribute paths to scan. Defaults cover derive_more 1.x.
attribute_paths = ["display", "debug"]

# When true (default), inline named-field references like `field`
# alongside positional `_0`/`_1`.
inline_named_fields = true

# When true (default), inline positional references `_0`, `_1`, etc.
inline_positional = true
```

A project that prefers the explicit-argument form for *named* fields
but the inlined form for positional ones can flip
`inline_named_fields = false`.

## Implementation notes

- `EarlyLintPass::check_attribute` reading the raw token stream so
  the format string and arguments are available pre-expansion.
- Match the attribute path against `attribute_paths`. The match
  uses the trailing segment only, so `#[display(...)]` and
  `#[derive_more::display(...)]` both qualify.
- **Parser style.** Implement the format-string scanner as
  parser-combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
  `take_literal_text` (everything up to the next `{`),
  `take_placeholder` (a `{...}` block, returning its position
  argument and format spec), `take_escaped_brace` (`{{` and `}}`).
  Composing these yields a stream of tokens whose argument-mapping
  pass is straightforward — far easier to maintain than the
  equivalent regex.
- Reuse the parser between this rule and any future rule that
  inspects `format!`-shaped attributes (e.g., a future
  `derive_more_format_spec_check` lint). Factor the helper
  crate-internally.
- The argument list is an `&[NestedMeta]` (or its post-attr-tokens
  equivalent). For each entry, classify with a small helper
  `is_simple_ident(tokens) -> Option<Symbol>`. Reject anything that
  is not a single identifier token surrounded by no other punctuation.
- Span construction: the autofix replaces the entire attribute span
  with the rewritten attribute text. This keeps the span math
  simple and avoids partial edits that confuse `cargo clippy --fix`.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Default state

Active by default.

## Autofix

`MachineApplicable`. The rewrite is a straightforward textual
substitution — every other side of the mapping is left exactly as
it was.

## Interaction with `clippy::uninlined_format_args`

The two lints together cover the full surface:

- `clippy::uninlined_format_args` — `format!`, `println!`, `write!`,
  `panic!`, etc.
- `derive_more_inlined_args` (this rule) — `#[display(...)]`,
  `#[debug(...)]`, and any other format-shaped derive_more
  attributes.

Enabling both gives a project consistent inlined-format-args style
across both source positions. Neither subsumes the other; they look
at different syntactic surfaces.
