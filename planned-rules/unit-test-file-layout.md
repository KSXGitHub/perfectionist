# `unit_test_file_layout`

**Sources:** parallel-disk-usage *Unit Tests › Where the external file sits*;
pacquet *Unit test file layout*.

## Statement

External unit-test modules live at `<parent>/tests.rs`. Specifically:

- For `src/foo.rs` the tests file is `src/foo/tests.rs`.
- For `src/foo/bar.rs` the tests file is `src/foo/bar/tests.rs`.

Do not flatten the tests into `src/foo_tests.rs`, and do not skip the
intermediate directory.

## What to lint

For every `mod tests;` (an external module declaration, not inline) carrying
`#[cfg(test)]`, resolve the file the compiler would load and verify:

1. The resolved path is `<parent_dir>/<parent_stem>/tests.rs`.
2. There is no sibling `<parent_stem>_tests.rs` next to the parent file.

If `mod tests { ... }` is used inline, this rule does not fire — the inline
form is permitted (parallel-disk-usage explicitly allows it; pacquet bans
it, which is captured separately if a project opts in).

## Examples

```rust
// src/foo.rs
#[cfg(test)]
mod tests; // OK if file is src/foo/tests.rs
```

```text
# Bad layouts
src/foo_tests.rs               # sibling, not nested
src/tests/foo.rs               # central tests directory
```

## Implementation notes

- `EarlyLintPass::check_item` on `ItemKind::Mod(.., ModKind::Loaded(..,
  Inline::No, ..))` gives access to the loaded `SourceFile` via
  `cx.sess().source_map().lookup_source_file(span.lo())`.
- Compute the expected path from the *parent* `SourceFile` (also
  retrievable through the source map). Compare with the resolved file's
  real path using `Path::ends_with`.
- Detect the sibling anti-pattern by walking the parent directory once per
  crate (cache it) and flagging any `*_tests.rs` whose stem matches a
  loaded module file.

## Severity

Warn. Auto-fix is non-trivial because it requires moving a file; emit a
help span pointing at the expected location instead.
