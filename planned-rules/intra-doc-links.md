# `intra_doc_links`

**Source:** pacquet *Doc comment intra-links*.

## Statement

> When a doc comment mentions an identifier that is intra-linkable from
> the current scope, write the mention as a rustdoc intra-doc link
> (`` [`Foo`] ``) rather than as bare prose (`` `Foo` ``).

## What to lint

For every `///` or `//!` doc comment, parse out backticked identifiers
and, for each, attempt to resolve the name in the documented item's
scope. If resolution succeeds and the identifier *is not* already wrapped
as an intra-doc link, suggest the link form.

Resolution rules:

- Skip identifiers that obviously refer to something rustdoc cannot link:
  shell command names, file paths (contain `/` or `.`), all-caps
  shouting, leading punctuation, etc.
- Skip well-known non-Rust tokens (`null`, `true`, `false`, `Bash`,
  `JSON`, etc.) via a configurable allowlist.
- Resolve through the *item's* surrounding scope, not the crate root, so
  that `[Foo]` works in a private module that has `use crate::foo::Foo;`.

## Examples

```rust
// Bad
/// Installs the package described by `PackageManifest` into `Store`.
pub fn install(manifest: &PackageManifest, store: &Store) { /* ... */ }

// Good
/// Installs the package described by [`PackageManifest`] into [`Store`].
pub fn install(manifest: &PackageManifest, store: &Store) { /* ... */ }
```

## Implementation notes

- `LateLintPass::check_attribute` or `check_item` to read each
  doc-comment attribute (`#[doc = "..."]`).
- Concatenate doc lines, then walk the markdown looking for inline code
  spans (`` `...` ``). Intra-doc-link patterns
  (`` [`Foo`] ``, `` [Foo] ``, `` [Foo](crate::foo::Foo) ``) are
  excluded.
- Resolve each candidate ident via `clippy_utils::path_to_local`-style
  lookups, or directly via `tcx.lookup_resolutions`. Rustdoc's own
  resolver is non-trivial to reuse; the practical compromise is to ask
  rustc's resolver in late-pass for the symbol in the current
  `ParentScope`.
- The autofix wraps the existing backticked span with `[` / `]`.

## Caveats

False positives: backticks around an identifier that the writer
deliberately *did not* want to link (e.g., a future type, or a
historical reference). Provide a project-level allowlist
(`intra_doc_links.skip_idents = ["LegacyCache"]`) and respect
`#[allow(...)]`.

## Severity

Warn. The autofix is `MachineApplicable` only when resolution is
unambiguous in the current scope.
