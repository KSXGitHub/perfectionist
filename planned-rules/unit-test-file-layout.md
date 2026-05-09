# `unit_test_file_layout`

**Sources:** parallel-disk-usage *Unit Tests › Where the external file
sits*; pacquet *Unit test file layout*. The two source documents
**diverge** on the inline-vs-external question, so this rule exposes
both styles as a configuration knob.

## Statement

The rule has two independent axes:

1. **Inline vs external (`mod foo;` as a file vs `mod foo { ... }`
   inline):** parallel-disk-usage allows inline test modules when they
   are short and only requires the move to an external file once the
   block grows long. pacquet requires every test module to live in an
   external file.
2. **External-file location:** when a `#[cfg(test)] mod <name>;` *is*
   external, where on disk does the file live? The two source documents
   agree on a nested layout (`src/foo/<name>.rs` for tests of
   `src/foo.rs`), but a project may legitimately accept the flat
   sibling form (`src/foo_<name>.rs`) or skip the intermediate
   directory.

The lint is **agnostic to the module's identifier**. A test submodule
may be named `tests`, `edge_cases`, `regression`, or anything else; the
layout rules apply to whatever name the project picks.

## Configuration

```toml
# dylint.toml
[unit_test_file_layout]

# How inline test modules are handled.
inline_style = "preserve"
# "external_only"      — every `#[cfg(test)] mod X;` must be external
#                        (matching pacquet's strict policy).
# "external_when_long" — inline allowed up to the configured threshold;
#                        beyond that, must be moved to a file
#                        (matching parallel-disk-usage's guidance).
# "preserve"           — no preference about inline vs external.

# Threshold for `external_when_long`. The inline block must satisfy
# *both* limits; the lint fires when either is exceeded. The line
# count is the source-line span of the inline `{ ... }` block,
# including braces. The percentage is `(inline_lines / file_lines) *
# 100`, where `file_lines` is the total line count of the parent
# source file.
#
# Defaults are set so that `inline_max_lines` is the active constraint
# in typical projects; bump or drop `inline_max_percent_of_file` to
# add the relative cap.
inline_max_lines = 50
inline_max_percent_of_file = 100   # 100 = effectively disabled

# How external test files must be laid out on disk.
external_layout = "nested"
# "nested"    — for `src/foo.rs` declaring `mod bar;` (test or otherwise),
#               the file must be `src/foo/bar.rs`. This matches both
#               source documents and is the strict default.
# "sibling"   — accepts the flattened `src/foo_bar.rs` form.
# "any"       — accepts whichever path Cargo would load. The lint then
#               only enforces inline-style and the no-sibling rule.

# When `external_layout = "nested"`, also flag any flat sibling that
# happens to exist for a module with the same name. Defaults to true;
# set false to skip the on-disk scan when the project mixes styles
# during a migration.
flag_unexpected_sibling = true

# Names that the lint should treat as "test submodules" for the
# purposes of `inline_style`. By default, any `#[cfg(test)] mod X;`
# qualifies regardless of `X`. This knob exists for projects that
# want the inline-style rule to apply only to specific module names
# (e.g., `["tests"]`) — leave empty to apply to every cfg-test module.
test_module_names = []
```

## What to lint

For every external module declaration `mod <name>;` (`ItemKind::Mod`
with `ModKind::Loaded(.., Inline::No, ..)`) that carries
`#[cfg(test)]`:

1. **Layout (per `external_layout`)**:
   - `nested`: resolved file path must be
     `<parent_dir>/<parent_stem>/<name>.rs`. Anything else — including
     `<parent_dir>/<name>.rs` (skipped intermediate) and
     `<parent_dir>/<parent_stem>_<name>.rs` (flattened sibling) — is
     flagged.
   - `sibling`: resolved file path must be either
     `<parent_dir>/<parent_stem>/<name>.rs` or
     `<parent_dir>/<parent_stem>_<name>.rs`. The latter is accepted
     even when the nested form does not exist.
   - `any`: layout check is skipped.
2. **Unexpected sibling (per `flag_unexpected_sibling`)**: if the
   resolved file is the nested form, scan the parent directory once
   per crate and flag any `<parent_stem>_<name>.rs` whose stem matches
   a loaded test module name. This catches stragglers from a
   half-completed migration.

For every inline test module
(`ItemKind::Mod` with `ModKind::Loaded(.., Inline::Yes, ..)` that
carries `#[cfg(test)]`):

3. **Inline-style (per `inline_style`)**:
   - `external_only`: emit unconditionally; suggest extracting to
     `<parent>/<name>.rs`.
   - `external_when_long`: count the line span of the inline `{ ... }`
     body, then compute its share of the parent file's total line
     count. Emit when *either* the absolute count exceeds
     `inline_max_lines` *or* the share exceeds
     `inline_max_percent_of_file`. The diagnostic names which limit
     was tripped so the author knows which threshold to consult.
   - `preserve`: emit nothing.

## Examples

### Inline-style alternatives

```rust
// src/foo.rs
//
// Acceptable when `inline_style = "preserve"` or
// `inline_style = "external_when_long"` and the body is short.
#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_empty_input() {
        assert_eq!(parse(""), Ok(vec![]));
    }
}
```

```rust
// src/foo.rs
//
// Required when `inline_style = "external_only"`, regardless of length.
// Also required under `external_when_long` once the body crosses the
// configured line budget.
#[cfg(test)]
mod tests;
```

### Layout alternatives (external, name = `tests`)

```text
# external_layout = "nested" (default)
src/foo.rs                   declares  mod tests;
src/foo/tests.rs             holds the test code

# external_layout = "sibling"
src/foo.rs                   declares  mod tests;
src/foo_tests.rs             also acceptable
```

### Layout alternatives (external, custom name)

```rust
// src/foo.rs
#[cfg(test)]
mod edge_cases;
```

```text
# Acceptable under any layout style:
src/foo/edge_cases.rs

# Under `external_layout = "sibling"` also:
src/foo_edge_cases.rs
```

The module's identifier is irrelevant to the layout rule; only the
file's position relative to its parent matters.

## Implementation notes

- `EarlyLintPass::check_item` to read the `cfg(test)` attribute and
  the `Inline` discriminant before macro expansion strips them.
- For external modules, locate the loaded `SourceFile` via
  `cx.sess().source_map().lookup_source_file(span.lo())` and compare
  with the parent file's path, both reduced to absolute `PathBuf`s.
- The unexpected-sibling scan runs once per parent directory; cache
  results keyed by `<parent_dir>` for the lifetime of the lint pass.
- Line counting for `external_when_long`: take the `Span` of the
  inline body and call `cx.sess().source_map().span_to_lines(span)`;
  the resulting `FileLines.lines.len()` is the inline-block count.
  Use `SourceFile::count_lines()` on the parent file (also reachable
  through the source map) for the denominator of the percentage check.
  Cache the per-file total per crate to avoid recounting.

## Severity

Warn for every sub-violation. Autofix:

- For inline-style violations under `external_only` and
  `external_when_long`, the fix requires creating a new file and
  replacing the inline body with `mod <name>;`. Suggest as help text;
  do *not* offer a `MachineApplicable` rewrite (a Dylint pass cannot
  create files).
- For layout violations, suggest the canonical path in help text.
  No `MachineApplicable` rewrite.
- For unexpected-sibling diagnostics, suggest deletion or merging the
  sibling into the canonical location. Help text only.
