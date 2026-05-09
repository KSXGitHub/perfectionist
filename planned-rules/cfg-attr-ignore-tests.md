# `cfg_attr_ignore_tests`

**Sources:** parallel-disk-usage *Conditional Test Skipping*; pacquet
*Conditional Test Skipping*.

## Statement

When a test cannot run under a given configuration, prefer
`#[cfg_attr(<predicate>, ignore = "...reason...")]` over `#[cfg(<predicate>)]`
so the test still compiles and is only skipped at runtime. Use `#[cfg]` only
when the test body genuinely cannot compile in the excluded configuration.
Provide a reason string in the `ignore` attribute.

## What to lint

Two related sub-lints.

### `cfg_attr_ignore_tests::should_be_cfg_attr`

For every `#[test]` item whose attribute list contains `#[cfg(predicate)]`,
attempt to detect whether the body uses items that are *themselves* gated
by a matching `#[cfg]`:

- Walk the body for path expressions.
- Resolve each path's `DefId` and check whether any ancestor module or
  the item itself carries a matching `cfg`.
- If no such reference exists, the test would compile under the
  excluded configuration; flag it and suggest replacing with
  `#[cfg_attr(predicate, ignore = "...")]`.

### `cfg_attr_ignore_tests::missing_reason`

For every `#[cfg_attr(<pred>, ignore)]` on a `#[test]` item, require the
`ignore` form `ignore = "<reason>"`. Flag bare `ignore` with no reason.

## Examples

```rust
// Bad: skip via cfg, but body uses only cross-platform types
#[cfg(unix)]
#[test]
fn unix_path_logic() {
    assert_eq!(Path::new("/a/b").display().to_string(), "/a/b");
}

// Good
#[test]
#[cfg_attr(not(unix), ignore = "only one path separator style is tested")]
fn unix_path_logic() {
    assert_eq!(Path::new("/a/b").display().to_string(), "/a/b");
}
```

```rust
// Acceptable: body genuinely cannot compile on non-unix
#[cfg(unix)]
#[test]
fn unix_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let p = std::fs::Permissions::from_mode(0o644);
    let _ = p.mode();
}
```

## Implementation notes

- `LateLintPass::check_item` for `ItemKind::Fn` items with the `#[test]`
  attribute (recognised via the `tcx.has_attr(.., sym::test)` helper or by
  matching the `rustc_attr_data_structures::AttributeKind`).
- The "would compile elsewhere" analysis is an over-approximation. Walk
  the body via the HIR visitor and resolve each `Res::Def` to its
  `DefId`; for each, walk the def-path upward via
  `tcx.parent`/`tcx.opt_local_def_id_to_hir_id` and inspect attributes
  for a `cfg` whose predicate is *not* a superset of the test's
  predicate. If any such item exists, mark the test as un-portable and
  do not lint.
- Detection of `cfg`s on extern items (e.g., `libc::stat`) requires
  reading metadata; `clippy_utils::tcx::Diagnostics`-style probes work,
  but the simplest correct fallback is to *only* lint when the body
  contains no path resolved to an item outside the local crate. That
  keeps false positives low at the cost of missing some opportunities.

## Severity

Warn.
