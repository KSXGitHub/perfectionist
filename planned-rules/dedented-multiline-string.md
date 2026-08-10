# `dedented_multiline_string`

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
clean line, and every later line is a literal newline in the value.
Its leading whitespace lands in the value too, so to keep the value
clean the author pushes the body flush-left and the literal visually
"falls out" of the function. Raw strings take the same shape:

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

- **A std-only inline form** is `concat!`, one quoted line per
  source line, each ending in `\n` as needed:

  ```rust
  let justfile = concat!(
      "clean:\n",
      "    rm -rf ./dist\n",
      "\n",
      "build:\n",
      "    tsc\n",
  );
  ```

- **A JSON document** is better built with `serde_json::json!`
  (the domain of `perfectionist::manual_json_string`).

- **A single logical line** that only *looks* multi-line because a
  trailing newline was spelled with a raw break collapses back to
  one source line:

  ```rust
  let nvmrc = "24.0.0\n";
  ```

This rule fires on the dedented shape and points at the
appropriate replacement. It is the source-layout complement of
`perfectionist::escaped_multiline_string`, which targets the
opposite spelling — newlines crammed onto one source line as `\n`
escapes (see [Interaction with sibling rules](#interaction-with-sibling-rules)).

## Why restrict this?

This is a stylistic preference, not a correctness issue. The
dedented literal compiles and produces exactly the intended
bytes. The objection is to the *source*:

- A reader scanning the indentation to follow control flow loses
  the thread the moment the body drops to column zero — the literal
  reads as if the function ended.
- The dedented body cannot be re-indented by `cargo fmt` or an
  editor reformat without corrupting the value: rustfmt does not
  touch a string literal's interior.
- For genuine file fixtures, keeping the content inline forgoes
  syntax highlighting, format-specific tooling, and a diff that
  reads as a change to *that file*.

Secondary: each body line's leading whitespace is part of the
value, so re-indenting the source to "fix" the layout silently
changes any whitespace-significant string (a justfile recipe body,
YAML).

## What to lint

For every string literal (`ExprKind::Lit` of `LitKind::Str` or
`LitKind::StrRaw`) whose **source spelling contains at least one
raw line break** — a `\n` byte inside the literal token that is
*not* a `\<newline>` line-continuation escape (which rustc elides),
so it is reflected as a newline in the decoded value — and where at
least one body line after such a break begins at a column
**shallower than the line on which the literal opens**:

1. Skip the literal if it is in a context the sibling string-literal
   rules also exempt (the lists mirror
   `perfectionist::escaped_multiline_string`):
   - The first positional argument of a recognised format-family
     macro (`format!`, `println!`, `panic!`, the `assert*` /
     `debug_assert*` family, the `log::*!` family, …): the literal
     is a template the macro interprets. Configurable via
     `format_macros`.
   - Already inside a `text_block!` / `text_block_fnl!` invocation
     or an `include_str!` / `include_bytes!` argument — avoids firing
     on already-fixed code and on path arguments.
   - Inside any attribute meta-item (`#[doc = "…"]`,
     `#[display("…")]`, `#[error("…")]`, …) — the literal is
     consumed by the attribute and reshaping it is not equivalent.
2. Emit the diagnostic with each **enabled** suggestion attached
   (toggled in config — see below), chosen by the literal's **shape**
   with no content parsing:
   - **One non-empty line** (every line but one is empty — e.g.
     `"24.0.0\n"` spread across source lines) → the single-line
     collapse, preserving the trailing `\n`(s).
   - **Multiple non-empty lines** → `include_str!` of a sibling
     fixture file, the `text_block_fnl!` / `text_block!` form, and
     `concat!` (one quoted line per line, each ending in `\n` as
     needed).

   Applicability is keyed to the toggles, **not** the individual fix:
   a structured suggestion is `MachineApplicable` only when exactly
   one suggestion is enabled (so `cargo dylint --fix` applies that one
   form deterministically). With two or more enabled, every
   suggestion is `MaybeIncorrect`, so `--fix` leaves the choice to the
   author instead of silently rewriting to one of them.

The rule inspects only layout, never content: a dedented JSON block
fires here on its layout and, independently, on
`perfectionist::manual_json_string` for its construction (see
[Interaction](#interaction-with-sibling-rules)). A literal whose
newlines are all `\n` escapes on one source line has no raw line
break and is the domain of `perfectionist::escaped_multiline_string`;
a raw/`\<newline>` literal whose body is indented to match the
surrounding code is not dedented and does not fire — see the
gen-docs boundary case below.

## Examples

### Dedented block, foreign format

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

### Dedented raw string holding JSON

**Avoid:**

```rust
let tsconfig = r#"
{
    "compilerOptions": { "strict": true },
    "include": ["**/*.ts"]
}
"#;
```

This fires on the dedented layout; the remedies are
`include_str!("fixtures/create/tsconfig.json")` or a `text_block_fnl!`
block. (`perfectionist::manual_json_string` fires separately on the
same literal to suggest `serde_json::json!` — see Interaction.)

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

- `tests/import_grouping_mismatch_submodules.rs` — nine dedented
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

Nothing here is dedented, so the rule stays silent. (The body
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
["perfectionist::dedented_multiline_string"]
# Which fixes the diagnostic offers. All on by default, so the
# diagnostic shows them as alternatives and `cargo dylint --fix`
# applies none (each is `MaybeIncorrect`). Set exactly one to `true`
# (the rest `false`) to make that form the single `MachineApplicable`
# fix `--fix` applies. (`include_str` is a help note — it needs a new
# file — so it is never auto-applied even when it is the only one on.)
suggest_include_str = true   # multi-line: extract to a sibling file
suggest_text_block  = true   # multi-line: text_block_fnl! / text_block!
suggest_concat      = true   # multi-line: concat!("line\n", …)
suggest_single_line = true   # one non-empty line: collapse to "…\n"

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
- **Indentation measurement.** With `lookup_char_pos`, find the
  column where the literal's line opens (its first non-whitespace
  byte) and each body line's leading-whitespace column. Fire when at
  least one body line is shallower. At column-zero indentation
  nothing is shallower, so a top-level flush-left `const` does not
  fire — the least-offensive case, left out deliberately.
- **Shape classification.** Split the decoded value on `\n`; if
  exactly one line is non-empty, take the single-line-collapse
  branch, otherwise the multi-suggestion branch. No content parsing
  (so no `serde_json` in this pass).
- **Skip contexts.** Reuse the sibling rule's machinery: a
  `Span::from_expansion()` check against `format_macros` (plus a
  built-in `text_block!` / `include_str!` path set) for the macro
  cases, and a `tcx.hir_parents(...)` walk for the attribute case.
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
- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing
  (`perfectionist::*`) that every registered lint follows.

### Difficulty

**Medium.** Detecting a raw line break whose body is dedented is a
straightforward snippet scan against the decoded value and a
column comparison through the source map. The context-skip logic is
shared with `perfectionist::escaped_multiline_string`. The work
concentrates in the suggestions, all attached to one diagnostic:

- `text_block_macros`: split the decoded value on `\n`, emit one
  quoted line each (raw-quoting lines that contain `"`), choosing
  `text_block_fnl!` when the value ends in `\n` and `text_block!`
  otherwise. Always `MaybeIncorrect` — it assumes the
  `text-block-macros` dependency and may need an added `use`, so it
  stays MaybeIncorrect even when it is the only enabled suggestion.
- `concat!`: emit one quoted line literal per line, each ending in
  `\n` where the value has one. Std-only and value-preserving.
- single-line collapse: replace the source with a one-line `"…\n"`,
  preserving the trailing `\n`(s). Value-preserving.
- `include_str!` extraction is a **help note only** — it needs a new
  file, which a rustc suggestion cannot create.

## Default state

Active by default. The dedented shape is a broad, project-agnostic
readability regression. Suppress per-site with
`#[expect(perfectionist::dedented_multiline_string)]` or globally via
`[perfectionist].disable`.

## Interaction with sibling rules

Each rule fires on its own trigger; none defers to or suppresses
another.

- [`escaped-multiline-string`](./escaped-multiline-string.md) —
  targets the opposite spelling: newlines crammed onto one source
  line as `\n` escapes. A purely `\n`-escaped literal has no raw
  line break, so it never matches this rule; the two rarely meet.
- [`manual-json-string`](./manual-json-string.md) — a dedented JSON
  literal triggers **both**, by design: that rule rewrites the JSON
  *construction* (string → `json!`), this one flags the *source
  layout*. This rule does no content parsing.
