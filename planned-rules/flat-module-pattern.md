# `flat_module_pattern`

**Sources:** parallel-disk-usage *Module Organization*; pacquet *Module
Organization*.

## Statement

> Use the flat file pattern (`module.rs`) rather than `module/mod.rs` for
> submodules.

## What to lint

For every loaded source file in the crate, flag any whose path ends in
`/mod.rs` (other than the crate root, which is `lib.rs` / `main.rs`).

## Examples

```text
# Bad
src/foo/mod.rs

# Good
src/foo.rs           # parent declares `mod foo;`
src/foo/bar.rs       # nested submodule
```

## Implementation notes

- Implement as a `LateLintPass` and emit one diagnostic per offending file
  in `check_crate`. The HIR exposes the `SourceFile` for every loaded module
  via `tcx.sess.source_map().files()`; iterate them and check
  `file.name.local_path()`.
- Filter to `RealFileName::LocalPath` so generated and embedded files are
  ignored.
- Exclude paths whose stem is `lib` or `main` and whose parent directory is
  the crate root (i.e., `src/lib.rs` and `src/main.rs` are fine even though
  some build setups symlink them).
- Use `clippy_utils::diagnostics::span_lint` with the `SourceFile`'s outer
  span (the first byte of the file).

## Suggested fix

Move `src/foo/mod.rs` to `src/foo.rs`. Cargo and rustc accept either
arrangement, so the move is mechanical and reversible.

## Severity

Warn by default. The rule is style-only and cannot break compilation.
