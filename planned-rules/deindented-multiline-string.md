# `deindented_multiline_string`

**Source:** project convention.

## Statement

A string literal whose source spelling is broken across physical
lines by a **raw line break** — an un-escaped newline that is part
of the literal token — pushes its body out of the surrounding
indentation, flush against the left margin:

```rust
pub fn create_justfile() {
    let justfile = "\
clean:
    rm -rf ./dist

build:
    tsc
";
    write_file("/tmp/fixtures/justfile", justfile).unwrap();
}
```

The leading `"\` swallows the first break so the body starts on a
clean line, and every line after it is a literal newline in the
value. The body *has* to sit at column zero (any leading
whitespace would land inside the value), so the literal visually
"falls out" of the function. The same shape appears with raw
strings:

```rust
pub fn create_tsconfig() {
    let tsconfig = r#"
{
    "compilerOptions": {
        "rootDir": ".",
        "outDir": "dist"
    }
}
"#;
    write_file("/tmp/fixtures/tsconfig.json", tsconfig).unwrap();
}
```

Several cleaner shapes exist, and which one is best depends on
what the literal *is*:

- **A real file in a foreign format** (a justfile, a `tsconfig.json`,
  an `index.ts`) belongs in a sibling fixture file, pulled in with
  `include_str!`:

  ```rust
  let justfile = include_str!("fixtures/create/justfile");
  ```

- **An inline block that must stay in the `.rs`** reads cleanly as
  one quoted line per source line through the
  [`text-block-macros`](https://crates.io/crates/text-block-macros)
  crate, which re-indents with the code:

  ```rust
  use text_block_macros::text_block_fnl;

  let js = text_block_fnl! {
      "#! /usr/bin/env node"
      r#"console.log("Hello, World!");"#
  };
  ```

- **A JSON document** is better built with `serde_json::json!`
  (the domain of `perfectionist::manual_json_string`).

- **A single logical line** that only *looks* multi-line because a
  trailing newline was spelled with a raw break collapses back to
  one source line:

  ```rust
  let nvmrc = "24.0.0\n";
  ```

This rule fires on the de-indented shape and points at the
appropriate replacement. It is the source-layout complement of
`perfectionist::escaped_multiline_string`, which targets the
opposite spelling — newlines crammed onto one source line as `\n`
escapes (see [Interaction with sibling rules](#interaction-with-sibling-rules)).

## Why restrict this?

This is a stylistic preference, not a correctness issue. The
de-indented literal compiles and produces exactly the intended
bytes. The objection is to the *source*:

- A reader scanning the indentation to follow control flow loses
  the thread the moment the body drops to column zero; the literal
  reads as if the function ended. Nesting (a `match` arm, a closure
  inside a method chain) makes the discontinuity worse.
- The de-indented body cannot be moved, re-indented by an editor's
  reformat, or wrapped in another block without either corrupting
  the value or being left behind by the surrounding `cargo fmt` —
  rustfmt does not touch the interior of a string literal.
- For genuine file fixtures, keeping the content inline forgoes
  syntax highlighting, format-specific tooling, and a diff that
  reads as a change to *that file* rather than to a Rust string.

There is also a quiet footgun worth naming, though it is the
secondary motivation: raw line breaks bake every leading space on
each body line **into the value**. When the value's leading
whitespace is significant (a Makefile/justfile recipe body, YAML),
re-indenting the source to "fix" the de-indentation silently
changes the string. The cleaner forms make the value's whitespace
explicit and independent of source layout.

## What to lint

For every string literal (`ExprKind::Lit` of `LitKind::Str` or
`LitKind::StrRaw`) whose **source spelling contains at least one
raw line break** — a `\n` byte inside the literal token that is
*not* a `\<newline>` line-continuation escape (which rustc elides)
and is therefore reflected as content/newline in the decoded
value — and where at least one of the body lines following such a
break begins at a source column **strictly less than** the
enclosing statement's indentation column:

1. Skip the literal if it is in a context the sibling string-literal
   rules also exempt (the lists mirror
   `perfectionist::escaped_multiline_string`):
   - The first positional argument of a recognised format-family
     macro (`format!`, `println!`, `panic!`, the `assert*` /
     `debug_assert*` family, the `log::*!` family, …): the literal
     is a template the macro interprets. Configurable via
     `format_macros`.
   - Already inside a `text_block!` / `text_block_fnl!` invocation
     (configurable via `text_block_macros_paths`) or an
     `include_str!` / `include_bytes!` argument — avoids firing on
     already-fixed code and on path arguments.
   - Inside any attribute meta-item (`#[doc = "…"]`,
     `#[display("…")]`, `#[error("…")]`, …) — the literal is
     consumed by the attribute and reshaping it is not equivalent.
2. Pick the suggested remedy from the literal's **shape** — no
   content parsing, which keeps the trigger simple:
   - **Single logical line** (the decoded value has no interior
     newline — at most a single trailing `\n`) → suggest collapsing
     to one source line (`"24.0.0\n"`). This is the case the rule
     exists to catch even though it "obviously" should be one line.
   - **Otherwise** → suggest the configured inline `style`
     (`text_block_macros` by default), and — when
     `suggest_include_str` is on — additionally surface
     "extract to a sibling file and `include_str!` it" as a help
     note, since a de-indented block is usually a foreign-format
     fixture.

The rule does **not** inspect what the content *is*. A de-indented
block whose content happens to be JSON still fires here on its
layout, and `perfectionist::manual_json_string` independently fires
on the same literal to suggest the `json!` construction — the two do
different things and neither suppresses the other (see
[Interaction](#interaction-with-sibling-rules)).

The trigger is the **raw line break plus de-indentation**, not the
decoded newline count. A literal whose newlines are all `\n`
escapes on one source line does not span source lines and so does
not match this rule's trigger (it is the domain of
`perfectionist::escaped_multiline_string`). A raw/`\<newline>`
literal whose body is indented to *match or exceed* the enclosing
code (so nothing is de-indented) does not fire — see the gen-docs
boundary case below.

## Examples

### De-indented block, foreign format

**Avoid:**

```rust
pub fn create_justfile() {
    let justfile = "\
clean:
    rm -rf ./dist

build:
    tsc
";
    write_file("/tmp/fixtures/justfile", justfile).unwrap();
}
```

**Prefer:** move the content to `fixtures/create/justfile` and
include it —

```rust
pub fn create_justfile() {
    let justfile = include_str!("fixtures/create/justfile");
    write_file("/tmp/fixtures/justfile", justfile).unwrap();
}
```

or, to keep it inline, one quoted line per source line —

```rust
pub fn create_justfile() {
    let justfile = text_block_fnl! {
        "clean:"
        "    rm -rf ./dist"
        ""
        "build:"
        "    tsc"
    };
    write_file("/tmp/fixtures/justfile", justfile).unwrap();
}
```

### De-indented raw string holding JSON

**Avoid:**

```rust
let tsconfig = r#"
{
    "compilerOptions": { "strict": true },
    "include": ["**/*.ts"]
}
"#;
```

This rule fires on the de-indented layout and suggests its own
remedies — `include_str!("fixtures/create/tsconfig.json")` or a
`text_block_fnl!` block. Separately,
`perfectionist::manual_json_string` fires on the same literal and
suggests the `serde_json::json!` construction, which is the better
end state for JSON specifically:

```rust
let tsconfig = serde_json::json!({
    "compilerOptions": { "strict": true },
    "include": ["**/*.ts"],
})
.to_string();
```

Both diagnostics are expected; they address different defects (one
the source layout, one the hand-rolled JSON construction).

### Accidentally multi-line single line

**Avoid:**

```rust
let nvmrc = "\
24.0.0
";
```

**Prefer:**

```rust
let nvmrc = "24.0.0\n";
```

### Real occurrences in this repository

The pattern has already slipped into `perfectionist`'s own test
suite. Both files embed configuration and Rust-source fixtures as
flush-left literals — the `CONFIG` blocks even carry `\"` escapes
that a raw `text_block!` line or an `include_str!`-ed `.toml` would
shed:

- `tests/import_grouping_mismatch_submodules.rs` — nine de-indented
  blocks: a `const CONFIG: &str = "\` TOML block plus the `lib` /
  `separate` / `deep` Rust-source fixtures threaded into
  `run_project_with_sources_and_config`. For example:

  ```rust
  const CONFIG: &str = "\
  [perfectionist]
  enable = [\"import_grouping_mismatch\"]

  [\"perfectionist::import_grouping_mismatch\"]
  style = \"multi_block\"
  ";
  ```

  → either a sibling `fixtures/.../config.toml` behind
  `include_str!`, or a raw-line `text_block_fnl!` that drops the
  `\"` noise:

  ```rust
  const CONFIG: &str = text_block_fnl! {
      "[perfectionist]"
      r#"enable = ["import_grouping_mismatch"]"#
      ""
      r#"["perfectionist::import_grouping_mismatch"]"#
      r#"style = "multi_block""#
  };
  ```

- `tests/uncombined_self_import_submodules.rs` — the analogous
  `const CONFIG: &str = "\` TOML block.

### Boundary case — indented raw string is *not* flagged

`tools/gen-docs/src/extract/shared/tests.rs` embeds Rust-source
fixtures in raw strings whose body is **indented to match the
surrounding code**:

```rust
write(
    base.join("common.rs"),
    r#"
        struct One;
        struct Two;
    "#,
)
```

Nothing here is de-indented, so the rule stays silent. (The body
indentation is baked into the value, but the consumer re-parses it
as Rust where leading whitespace is irrelevant — a deliberate,
acceptable trade-off, not the anti-pattern this rule targets.)

### Skipped contexts

**Not flagged:**

```rust
// format template — the macro interprets it
println!("\
line one {x}
line two
", x = 42);

// attribute literal
#[doc = "\
first line
second line
"]
fn documented() {}

// already inside text_block!
let _ = text_block_fnl! { "foo" "bar" };

// path argument, not content
let s = include_str!("fixtures/create/justfile");
```

## Configuration

```toml
[perfectionist::deindented_multiline_string]
# Which inline rewrite the autofix offers as its primary
# suggestion. Defaults to `text_block_macros`.
#   "text_block_macros"  -> text_block_fnl! / text_block! (one
#                           quoted line per source line, re-indented
#                           with the code)
#   "line_continuation"  -> a single literal whose raw breaks become
#                           `\n\<newline><indent>` continuations,
#                           preserving the value while re-indenting
style = "text_block_macros"

# Whether to additionally surface "extract to a sibling file and
# pull it in with `include_str!`" as a help note. Defaults to
# `true`: a de-indented block is most often a foreign-format
# fixture that reads best as its own file. Set to `false` to keep
# the suggestion purely inline.
suggest_include_str = true

# Format-family macros whose first positional argument is a
# template and must not be reshaped. Mirrors
# `perfectionist::escaped_multiline_string`.
format_macros = [
  "format", "println", "eprintln", "format_args",
  "write", "writeln", "print", "eprint",
  "panic", "unimplemented", "todo", "unreachable",
  "assert", "assert_eq", "assert_ne",
  "debug_assert", "debug_assert_eq", "debug_assert_ne",
  "error", "warn", "info", "debug", "trace", "log",
]

# text-block-macros invocations whose arguments are already the
# rewritten form; skipped to avoid firing on the rule's own output.
text_block_macros_paths = [
  "text_block_macros::text_block",
  "text_block_macros::text_block_fnl",
]

# Import paths the `text_block_macros` autofix suggests, if no such
# import is already in scope. Override when the project re-exports
# the macros from an internal prelude.
text_block_import_path = "text_block_macros::text_block"
text_block_fnl_import_path = "text_block_macros::text_block_fnl"
```

## Implementation notes

- **Pass kind.** `LateLintPass::check_expr` on
  `ExprKind::Lit`. The decoded value comes from rustc
  (`lit.symbol_unescaped`); the *source spelling* needed to detect
  raw line breaks and to measure indentation comes from
  `cx.sess().source_map().span_to_snippet(lit.span)`.
- **Raw-line-break detection.** A raw line break is a newline that
  survives into the value. Compare source spelling against decoded
  value: walk the snippet's bytes, and treat a source `\n` as a raw
  break unless it is immediately preceded by an un-escaped `\`
  (a `\<newline>` continuation, which rustc elides). Raw strings
  (`r"…"`, `r#"…"#`) have no escapes, so every source `\n` inside
  them is a raw break. Implement this as a small `take_*` scanner
  over the snippet per the parser-combinator convention in
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md);
  do not reach for a regex.
- **De-indentation measurement.** Use the source map's
  `lookup_char_pos` on the literal's span to get the enclosing
  statement's indentation column (the column of the first
  non-whitespace byte on the literal's opening line). For each body
  line after a raw break, compute its leading-whitespace column.
  Fire only when at least one body line's column is strictly less
  than the statement indentation. At crate-root indentation (column
  zero) nothing is shallower, so a top-level `const` with a
  flush-left body does not fire — the least-offensive case, left
  out deliberately.
- **Shape classification.** Count interior newlines in the decoded
  value to pick the single-line-collapse vs. block-reshape branch.
  No content parsing — the rule never inspects whether the body is
  JSON, TOML, or anything else, which keeps it independent of
  `perfectionist::manual_json_string` and avoids pulling
  `serde_json` into this pass.
- **Skip contexts.** Reuse the sibling rule's machinery: a
  `Span::from_expansion()` check against `format_macros` /
  `text_block_macros_paths` for the macro cases, and a
  `tcx.hir_parents(...)` walk for the attribute case. `include_str!`
  / `include_bytes!` arguments are caught by the same
  expansion-path check.
- **Proc-macro suppression.** The diagnostic's primary span is the
  **whole literal**, which is wider than the synthesised-identifier
  spans the "Suppressing proc-macro-synthesised violations" section
  of [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  warns about. By the "vulnerable exactly when the diagnostic span
  is narrower than the offending node" test, this rule is **not**
  vulnerable: `declare_tool_lint! { … report_in_external_macro: false }`
  alone suffices and no `hir_in_external_macro` guard or
  `ui/<rule>_proc_macro.rs` fixture is required. Record this
  reasoning at the span-selection site so the omission reads as
  deliberate.

### Difficulty

**Medium.** The raw-line-break-plus-de-indentation detection is a
straightforward snippet scan against the decoded value and a
column comparison through the source map. The context-skip logic is
shared with `perfectionist::escaped_multiline_string`. The work
concentrates in the autofix:

- `text_block_macros` autofix: split the decoded value on `\n`,
  emit one quoted line each (raw-quoting lines that contain `"`),
  choose `text_block_fnl!` when the value ends in `\n` and
  `text_block!` otherwise — the same construction as
  `escaped_multiline_string`. `Applicability::MaybeIncorrect`
  (assumes the `text-block-macros` dependency and may need an added
  `use`).
- `line_continuation` autofix: rewrite each raw break to
  `\n\<newline><indent>` matching the statement column. Pure
  syntactic rewrite, `Applicability::MachineApplicable`.
- single-line collapse: replace the literal's source with a
  one-line `"…\n"`. `Applicability::MachineApplicable`.
- `include_str!` extraction is offered as a **help note only**, not
  a structured suggestion — it requires creating a new file, which
  a rustc suggestion cannot do.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

## Default state

Active by default. The de-indented shape is a broad, project-
agnostic readability regression; the default `style =
"text_block_macros"` matches the catalogue's preferred form, and
projects that don't want the external-crate dependency switch to
`line_continuation`. Suppress per-site with
`#[allow(perfectionist::deindented_multiline_string)]` or globally
via `[perfectionist].disable`.

## Interaction with sibling rules

These rules each do their own thing on their own trigger; none
suppresses, defers to, or coordinates with another. Cross-rule
avoidance would only complicate every implementation, so a literal
that happens to trip two of them simply gets two diagnostics — they
point at different defects, so both are appropriate.

- [`escaped-multiline-string`](./escaped-multiline-string.md) — the
  two rules target different **source spellings** of a multi-line
  string, so in practice they rarely fire on the same literal:
  - `escaped_multiline_string` targets newlines crammed onto one
    (or few) source lines as `\n` escapes — *too compressed*.
  - `deindented_multiline_string` targets newlines spelled as raw
    source breaks that drop the body out of the code's
    indentation — *too sprawling*.

  This rule's trigger is the raw-line-break-with-de-indentation
  shape; a purely `\n`-escaped literal does not match it at all.
- [`manual-json-string`](./manual-json-string.md) — a de-indented
  literal whose content is JSON triggers **both** rules, and that is
  intended. They address different defects: `manual_json_string`
  rewrites the *construction* (hand-rolled string → `serde_json::json!`),
  while this rule flags the *source layout* (the de-indented body).
  This rule does no content parsing, so it neither knows nor cares
  that the body is JSON.
