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
- [`import-granularity.md`](./import-granularity.md) — enforce a project-wide
  import-granularity style. Configurable as `crate` (the recommendation
  from both source documents — one nested `use` per crate root),
  `module` (one `use` per leaf module, items from the same module
  merged), or `item` (one `use` per leaf path). Names map 1-to-1 to
  rustfmt's `imports_granularity`.
- [`import-grouping.md`](./import-grouping.md) — enforce a project-wide
  import-grouping style. Configurable as `single_group` (every `use` in
  one contiguous block) or `grouped` (partitioned into std / internal /
  third-party blocks separated by blank lines, with the order
  configurable).
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
- [`em-dash-prose.md`](./em-dash-prose.md) — flag em dashes in doc comments
  and string literals reachable from `format!` / `println!` style macros.
- [`unicode-ellipsis-in-docs.md`](./unicode-ellipsis-in-docs.md) — flag
  U+2026 (`…`) in `///` and `//!` doc comments; prefer `...`.
- [`unicode-ellipsis-in-comments.md`](./unicode-ellipsis-in-comments.md) —
  flag U+2026 (`…`) in `//` and `/* */` comments; prefer `...`.
- [`unicode-ellipsis-in-panic-messages.md`](./unicode-ellipsis-in-panic-messages.md) —
  flag U+2026 (`…`) in `panic!` / `assert*!` / `expect` messages;
  prefer `...`.

### Clap derive help
- [`clap-help-no-markdown.md`](./clap-help-no-markdown.md) — forbid
  markdown constructs (HTML, links, intra-doc links, code blocks, code
  spans, headings) in doc comments that clap derive macros consume as
  help text. Disabled when the item carries an explicit help override
  (`#[arg(help = ...)]`, `#[clap(about = ...)]`, etc.).
- [`clap-help-length.md`](./clap-help-length.md) — flag clap-bound doc
  comments that exceed configurable line / character budgets (catches
  AI-generated bloat). Same override allowlist.

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
- **Doc comments referencing items more private than the documented item**
  (pacquet *Documentation comments*) — already covered by rustdoc's
  built-in `rustdoc::private_intra_doc_links` lint (default `warn`).
  Run `RUSTFLAGS='-D warnings' cargo doc --document-private-items` to
  promote it to a hard error. The bare-backtick variant
  (`` `Foo` `` rather than `` [`Foo`] ``) is funnelled into intra-doc
  links by the [`intra-doc-links`](./intra-doc-links.md) rule, after
  which rustdoc catches it. Reimplementing this in Dylint would be a
  less accurate duplicate of rustdoc's own resolver.
