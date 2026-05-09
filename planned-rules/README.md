# Planned Lints

This directory enumerates lints that `perfectionist` should provide. Each rule
is captured in its own file and was extracted from one or both of:

- [parallel-disk-usage `CONTRIBUTING.md`](https://github.com/KSXGitHub/parallel-disk-usage/blob/master/CONTRIBUTING.md)
- [pacquet `CODE_STYLE_GUIDE.md`](https://github.com/pnpm/pacquet/blob/main/CODE_STYLE_GUIDE.md)

Every rule listed here is judged feasible to detect with a Dylint
`LateLintPass` (or `EarlyLintPass`) backed by `clippy_utils`. Rules from the
source documents that are *not* mechanically checkable are listed at the bottom
of this file.

## Index

### Module and file layout
- [`flat-module-pattern.md`](./flat-module-pattern.md) — forbid `mod.rs` in
  favour of the flat `module.rs` layout.
- [`unit-test-file-layout.md`](./unit-test-file-layout.md) — external test
  modules must live at `<parent>/tests.rs`, never `<parent>_tests.rs` or
  flattened into a sibling.
- [`module-item-order.md`](./module-item-order.md) — within a module file,
  `pub mod` first, then `pub use`, then private items.

### Imports
- [`merged-imports.md`](./merged-imports.md) — collapse multiple `use`
  statements that share a prefix into a single braced `use`.
- [`no-star-imports.md`](./no-star-imports.md) — forbid `use foo::*` inside
  module bodies, with a documented allowlist for preludes and root re-exports.

### Naming
- [`single-letter-names.md`](./single-letter-names.md) — the umbrella rule for
  single-letter generics, `let` bindings, function parameters, and closure
  parameters, with the exact allow-list from both guides.

### Trait bounds and signatures
- [`where-clause-bounds.md`](./where-clause-bounds.md) — prefer `where` clauses
  over inline bounds when there are multiple constraints.

### Derives and error types
- [`derive-ordering.md`](./derive-ordering.md) — enforce the canonical order
  for `#[derive(...)]` and require splitting across lines by category.
- [`error-type-derives.md`](./error-type-derives.md) — `derive_more::Display` /
  `Error` must only be derived when actually needed; flag superfluous `Error`
  on non-error types.
- [`non-exhaustive-error.md`](./non-exhaustive-error.md) — public error enums
  should carry `#[non_exhaustive]`.

### Pipe trait
- [`unnecessary-pipe.md`](./unnecessary-pipe.md) — flag `.pipe(f)` where the
  receiver is not the tail of an existing method chain.

### Tests
- [`cfg-attr-ignore-tests.md`](./cfg-attr-ignore-tests.md) — prefer
  `#[cfg_attr(..., ignore = "...")]` over `#[cfg(...)]` on `#[test]`s, and
  require an `ignore` reason string.

### Cloning
- [`arc-rc-clone.md`](./arc-rc-clone.md) — require `Arc::clone(&x)` /
  `Rc::clone(&x)` instead of `x.clone()` when `x: Arc<_>` / `Rc<_>`.

### Serde
- [`serde-source-types.md`](./serde-source-types.md) — forbid
  `#[serde(from = "&'de str")]` / `try_from = "&'de str"`; advise
  `Cow<'de, str>` or `String`.

### Documentation
- [`intra-doc-links.md`](./intra-doc-links.md) — backticked identifiers in
  rustdoc comments that resolve in scope must be written as intra-doc links.
- [`private-doc-references.md`](./private-doc-references.md) — `///` and
  `//!` on a `pub` (or `pub(crate)`) item must not name an item that is more
  private than itself.
- [`em-dash-prose.md`](./em-dash-prose.md) — flag em dashes in doc comments
  and string literals reachable from `format!` / `println!` style macros.

## Out of scope (cannot be linted by Dylint)

The following rules from the source documents either describe processes,
external state, or judgement calls that a static lint cannot evaluate:

- **Conventional Commits format** for git messages — lives in commit
  metadata, not source. Use a `commit-msg` hook or CI check.
- **Run `cargo fmt`, `cargo clippy`, and `cargo test`** before submitting —
  CI / pre-commit concerns, not source patterns.
- **Build with no, default, and all features** — driven by CI matrix.
- **Writing-style rules** beyond em dashes (formal tone, complete sentences,
  no long parentheticals) — judgement-based prose review.
- **Inline vs external test module decision based on length** — any
  threshold is arbitrary; flagged here as an *advisory* lint candidate but
  rejected as a real rule. The hard rule (external file path layout) is
  covered by [`unit-test-file-layout.md`](./unit-test-file-layout.md).
- **Owned vs borrowed parameter trade-off** — `clippy::ptr_arg` already flags
  `&PathBuf` / `&String` / `&Vec<_>` parameters, which covers the
  "most encompassing type" case from the pacquet guide. The remaining
  trade-off ("would owned reduce total copies?") requires whole-program
  reasoning that a single lint pass cannot perform.
- **Reporter wire-contract requirements** in pacquet (channel naming,
  upstream permalink comments, ordering relative to side effects, recording
  fakes in tests) — these depend on cross-repo invariants and human
  judgement about the upstream call site.
- **Pattern-match wrapping style** — the "concise wrapping style" example
  is already idiomatic and there is no concrete anti-pattern to detect.
- **Test logging guidance** (`assert_eq!` with multi-line strings should
  log via `eprintln!`, complex structures via `dbg!`) — the rule branches
  on the runtime *shape* of values, which a static lint cannot inspect.
