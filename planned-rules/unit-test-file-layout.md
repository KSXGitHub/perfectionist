# `unit_test_file_layout`

**Sources:** parallel-disk-usage *Unit Tests › Where the external file
sits*; pacquet *Unit test file layout*. The two source documents
**diverge** on the inline-vs-external question, so this rule exposes
both styles as a configuration knob.

## Statement

The rule has two independent axes:

1. **Inline vs external:** test code can be intermingled with
   production code in the same file in two shapes — inline
   `#[cfg(test)] mod X { ... }` blocks, or bare `#[test] fn` /
   `#[cfg(test)] fn` / `#[cfg(test)] use` items declared next to
   production code. Both forms count as "inline test code". The rule
   measures the *total* inline-test footprint per file, not each
   block in isolation. parallel-disk-usage allows inline test code
   when its footprint is small; pacquet requires every test item to
   live in an external file.
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

# Threshold for `external_when_long`. The lint sums the line spans of
# every inline test item in a file (see `inline_test_kinds` below) and
# compares the total against both limits. The lint fires when either
# is exceeded. The percentage is `(inline_test_lines / file_lines) *
# 100`, where `file_lines` is the total line count of the parent
# source file.
#
# Defaults are set so that `inline_max_lines` is the active constraint
# in typical projects; bump or drop `inline_max_percent_of_file` to
# add the relative cap.
inline_max_lines = 50
inline_max_percent_of_file = 100   # 100 = effectively disabled

# Item kinds that count toward the inline-test footprint. The defaults
# cover every item that only exists in test builds, so a project's
# test footprint is correctly measured even when it does not use
# `mod tests { ... }` at all.
inline_test_kinds = [
  "cfg_test_mod",      # inline `#[cfg(test)] mod X { ... }`
  "test_fn",           # `#[test] fn ...` at module level
  "cfg_test_fn",       # `#[cfg(test)] fn ...` (test helpers)
  "cfg_test_other",    # `#[cfg(test)] struct/enum/use/const ...`
]

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

For every parent source file, scan the top-level items and collect
every **inline test item** — by default that is the union of:

- inline `#[cfg(test)] mod X { ... }` blocks,
- `#[test] fn ...` at module level,
- `#[cfg(test)] fn ...` (test helpers next to production code),
- any other `#[cfg(test)]`-gated item (`use`, `struct`, `const`, …).

The set is configurable via `inline_test_kinds`.

3. **Inline-style (per `inline_style`)**:
   - `external_only`: emit one diagnostic *per* collected inline test
     item. Each one is bad on its own and the suggested fix is to
     move all of them into an external `mod <name>;`.
   - `external_when_long`: sum the line spans of every collected
     inline test item in the file, then compute the sum's share of
     the parent file's total line count. Emit a single per-file
     diagnostic when *either* the absolute total exceeds
     `inline_max_lines` *or* the share exceeds
     `inline_max_percent_of_file`. The diagnostic spans the contiguous
     run of inline test items (or the union of their spans, if they
     are not contiguous), names which limit was tripped, and points
     at the canonical extraction target. A file that has only one or
     two short tests stays under the budget and is not flagged, even
     when it has no `mod tests { ... }` block at all.
   - `preserve`: emit nothing.

A file containing **only** test items (e.g., `src/foo/tests.rs`
itself, or any file whose path matches `external_layout`) is
exempt from the inline-style check — it is by definition the
extraction target, not the place a programmer is meant to extract
*from*.

## Examples

### Inline-style alternatives

```rust
// src/foo.rs
//
// Acceptable when `inline_style = "preserve"` or
// `inline_style = "external_when_long"` and the inline-test footprint
// is small.
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
// Same threshold check applies when there is no `mod tests` block at
// all and tests are written as bare items. The footprint counted by
// `external_when_long` is the sum of every `#[test] fn`, every
// `#[cfg(test)] fn`, and any other `#[cfg(test)]` items.
fn parse(input: &str) -> Result<Vec<Token>, ParseError> { /* ... */ }

#[test]
fn parses_empty_input() {
    assert_eq!(parse(""), Ok(vec![]));
}

#[cfg(test)]
fn fixture() -> String { /* ... */ }

#[test]
fn parses_full_input() {
    let input = fixture();
    assert!(parse(&input).is_ok());
}
```

```rust
// src/foo.rs
//
// Required when `inline_style = "external_only"`, regardless of
// length. Also required under `external_when_long` once the combined
// inline-test footprint crosses the configured budget.
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
- Walking the parent module: implement `external_when_long` as a
  per-`SourceFile` accumulator. In `EarlyLintPass::check_mod` (or
  `check_crate` walking each module body) iterate top-level items,
  classify each as production-or-test using the configured
  `inline_test_kinds`, and sum the per-item line spans for the
  test items. Emit once per parent source file when the sum exceeds
  either limit.
- Item classification:
  - `cfg_test_mod`: `ItemKind::Mod(.., Inline::Yes)` carrying
    `#[cfg(test)]`.
  - `test_fn`: `ItemKind::Fn` carrying `#[test]`.
  - `cfg_test_fn`: `ItemKind::Fn` carrying `#[cfg(test)]`.
  - `cfg_test_other`: any other `ItemKind` carrying `#[cfg(test)]`.
- Skip the inline-style check entirely for a file that contains
  *only* test items — that file is itself a valid extraction target.
  Detect by classifying every top-level item once and confirming the
  production-item count is zero.
- Line counting: for each contributing item, take its `Span` and call
  `cx.sess().source_map().span_to_lines(span)`; the
  `FileLines.lines.len()` is its line count. Sum across items. Use
  `SourceFile::count_lines()` on the parent file for the denominator
  of the percentage check. Cache the per-file total per crate to
  avoid recounting.

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
