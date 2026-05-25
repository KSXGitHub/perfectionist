# Planned Lints

This directory enumerates lints that `perfectionist` should provide. Each rule
is captured in its own file and was extracted from one or both of:

- [parallel-disk-usage `CONTRIBUTING.md`](https://github.com/KSXGitHub/parallel-disk-usage/blob/master/CONTRIBUTING.md)
- [pacquet `CODE_STYLE_GUIDE.md`](https://github.com/pnpm/pacquet/blob/main/CODE_STYLE_GUIDE.md)

Every rule listed here is judged feasible to detect with a Dylint
`LateLintPass` (or `EarlyLintPass`) backed by `clippy_utils`. Rules from the
source documents that are *not* mechanically checkable are listed at the bottom
of this file.

Cross-cutting implementation conventions — including the parser-combinator
pattern that several rules call out by reference — live in
[`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md).

## Index

### Module and file layout
- [`module-item-order.md`](./module-item-order.md) — within a module file,
  `pub mod` first, then `pub use`, then private items.

### Imports
- [`import-granularity.md`](./import-granularity.md) — enforce a project-wide
  import-granularity style. Configurable as `crate` (one nested `use`
  per crate root — the shape both source documents use in their
  examples), `module` (default; one `use` per leaf module, items
  from the same module merged), or `item` (one `use` per leaf path).
  Names map 1-to-1 to rustfmt's `imports_granularity`.
- [`import-grouping.md`](./import-grouping.md) — enforce a project-wide
  import-grouping style. Configurable as `single_group` (every `use` in
  one contiguous block) or `grouped` (partitioned into std / internal /
  third-party blocks separated by blank lines, with the order
  configurable).
- [`self-import.md`](./self-import.md) — decide how `self` in `use`
  statements is handled. Configurable as `forbid` (always prefer the
  bare `use foo::bar;`) or `combined` (fold adjacent module + item
  imports into `use foo::bar::{self, X};`). Inactive by default; opt
  in and pick a direction.
- [`core-or-std.md`](./core-or-std.md) — decide whether items that
  exist in both `core`/`alloc` and `std` should be named through the
  narrower or wider path. Configurable as `prefer_core` (matches
  `clippy::std_instead_of_core` + `std_instead_of_alloc`) or
  `prefer_std`. Inactive by default; opt in and pick a direction.
- [`no-star-imports.md`](./no-star-imports.md) — forbid `use foo::*` inside
  module bodies. Two exceptions are enabled by default and individually
  configurable: the prelude form (`use foo::prelude::*`) and root-of-
  module re-exports (`pub use submodule::*`). A project can disable
  either or both.
- [`named-prelude-import.md`](./named-prelude-import.md) — dual of the
  previous rule. Forbid named imports from a `prelude` module
  (`use foo::prelude::Item;`); allow the glob form
  (`use foo::prelude::*;`).

### Naming
- [`qualified-paths.md`](./qualified-paths.md) — decide whether items from
  outside the current scope are named by their full path
  (`std::fs::create_dir_all`, `#[derive(clap::Parser)]`) or imported
  via `use` and called by the simple identifier. AI tends to produce
  the former; parallel-disk-usage prefers the latter. Inactive by
  default; opt in and pick `unqualified` or `qualified`.

### Trait bounds and signatures
- [`where-clause-bounds.md`](./where-clause-bounds.md) — prefer `where` clauses
  over inline bounds when there are multiple constraints.
- [`prefer-owned-parameter.md`](./prefer-owned-parameter.md) — when a
  function takes `&T` but the body unconditionally calls
  `.to_owned()` / `.to_path_buf()` / equivalent, take `T` directly.
  Pairs with `clippy::ptr_arg` and `clippy::needless_pass_by_value`
  to cover the full owned-vs-borrowed trade-off from the pacquet
  guide.

### Derives and error types
- [`error-type-derives.md`](./error-type-derives.md) — derive
  `derive_more::Display` / `Error` only when needed; flag superfluous
  `Error` and types that violate the project's error-naming convention.
- [`prefer-derive-more.md`](./prefer-derive-more.md) — flag hand-written
  `impl` blocks that could be replaced by a `derive_more` derive
  (`From`, `Into`, `AsRef`, `Deref`, etc., with `Display` and
  `Error` available behind opt-in flags due to detection difficulty).
- [`derive-more-inlined-args.md`](./derive-more-inlined-args.md) —
  `clippy::uninlined_format_args` for `#[display(...)]` and
  `#[debug(...)]` attributes from `derive_more`.

### Pipe trait
- [`pipe-style.md`](./pipe-style.md) — bidirectional pipe-trait
  policy. Flags `value.pipe(f)` at the start of a chain (suggests
  `f(value)`) and flags `f(chain)` wrapping a method chain
  (suggests `chain.pipe(f)`). Both checks default to enforce.

### Tests
- [`cfg-attr-ignore-tests.md`](./cfg-attr-ignore-tests.md) — prefer
  `#[cfg_attr(..., ignore = "...")]` over `#[cfg(...)]` on `#[test]`s, and
  require an `ignore` reason string.
- [`prefer-json-macro.md`](./prefer-json-macro.md) — in test
  code, when a string literal or `format!` invocation produces a
  non-trivial JSON document, prefer
  `serde_json::json!({ ... }).to_string()`. By default silent in
  production code (performance trade-off is reviewer's call;
  `restrict_to_tests = false` opts in) and in workspaces that
  do not depend on `serde_json` anywhere
  (`require_serde_json_dependency = false` opts in).

### String literals
- [`prefer-text-block.md`](./prefer-text-block.md) — when a string
  literal contains 2+ embedded `\n` newlines (and isn't a format
  template or display-attribute), prefer `text_block! { ... }` /
  `text_block_fnl! { ... }` (default) or the
  `"line\n\<newline>line"` continuation form. Skips templates and
  attribute literals.
- [`print-macro-split.md`](./print-macro-split.md) — when a
  splittable print macro (`println!`, `eprintln!`, `writeln!`,
  log family, …) has an embedded-`\n` template *and* spans more
  than `max_line_width` columns, suggest either splitting into
  one call per line (`multiple_calls`, default) or folding the
  template with backslash-newline continuations
  (`line_continuation`). Excludes `format!`/`format_args!` and
  the panic/assert family because their behaviour changes under
  splitting.
- [`format-macro-wrap.md`](./format-macro-wrap.md) — counterpart
  for the *unsplittable* macros: `format!`, `format_args!`,
  `panic!`, `assert!` (with message), the `debug_assert*` family,
  `unimplemented!`/`todo!`/`unreachable!`. When the source line
  exceeds `max_line_width`, suggest folding the template with
  `\n\<newline>` continuations. Only one rewrite — multi-call is
  not viable for these macros.
- [`derive-more-template-wrap.md`](./derive-more-template-wrap.md)
  — same width-driven wrapping for derive_more attribute-form
  templates: `#[display(...)]`, `#[debug(...)]`. The attribute
  is consumed by a derive macro so multi-attribute splitting
  isn't viable; only line-continuation rewriting.

### Macros
- [`macro-trailing-comma.md`](./macro-trailing-comma.md) — apply
  rustfmt's `trailing_comma = "Vertical"` policy (trailing comma
  on multi-line argument lists, none on single-line) to
  function-like macro invocations (`ast::MacCall`), which
  rustfmt itself leaves untouched. Attribute-style invocations
  like `#[derive(...)]` are out of scope. Two eligibility modes:
  **name-based**, a curated list of core/std and well-known
  third-party macros (easy, configurable via
  `extra_macros`), and **matcher-based**, an automatic walk
  of `macro_rules!` matchers to detect the `$(,)?` / `$(,)*`
  optional-trailing-comma idioms (harder, not yet implemented
  and therefore not configurable).
- [`macro-argument-binding.md`](./macro-argument-binding.md) —
  require impure expressions passed to function-like and
  array-like macro invocations to be bound to a `let` first.
  Targets the `debug_assert_eq!(map.insert(k, v), None)` footgun
  (release builds skip evaluation entirely) and the general
  class of "macros don't promise exactly-once argument
  evaluation". Four `mode` values ordered by implementation
  cost: a tiny deny set of `debug_assert*`, an opt-in blanket
  ban, the default curated allow + deny sets, and
  matcher-based analysis for unknown `macro_rules!` macros that
  counts each `$name:expr` capture's `$name` references in the
  expansion. Curly-brace invocations are out of scope.

### Serde
- [`serde-source-types.md`](./serde-source-types.md) — forbid
  `#[serde(from = "&'de str")]` / `try_from = "&'de str"`; advise
  `Cow<'de, str>` or `String`.
- [`serde-wrapper-style.md`](./serde-wrapper-style.md) — when a
  single-field wrapper has trivial `From` / `Into` impls,
  `#[serde(transparent)]` and `#[serde(from = "T", into = "T")]`
  produce the same wire format. The lint enforces a project-wide
  choice between the two (`transparent` for zero-cost,
  `from_into` to keep a validation hook ready). Inactive by
  default; opt in and choose `transparent` or `from_into`.

### Documentation
- [`intra-doc-links.md`](./intra-doc-links.md) — backticked identifiers in
  rustdoc comments that resolve in scope must be written as intra-doc links.
- [`em-dash-prose.md`](./em-dash-prose.md) — flag em dashes in doc comments
  and string literals reachable from `format!` / `println!` style macros.
- [`unpinned-repo-ref.md`](./unpinned-repo-ref.md) — require URLs that
  reference files in a hosted git repository (GitHub, GitLab,
  Bitbucket, Codeberg / Gitea, sourcehut, …) to be pinned to a
  commit SHA, with optional acceptance of tag refs. Branch refs
  like `/blob/main/...` are rejected.
- [`commit-id-length.md`](./commit-id-length.md) — enforce a
  consistent SHA length for commit IDs that appear in forge URLs.
  Covers file references, single-commit views (`/commit/<sha>`),
  and range comparisons (`/compare/<sha>...<sha>`). Defaults are
  permissive (any length passes); a project tightens the window to
  pin a fixed length such as 12 or 40.

### Lint-level attributes
- [`prefer-expect-over-allow.md`](./prefer-expect-over-allow.md) —
  rewrite `#[allow(...)]` to `#[expect(...)]` for lints that fire
  deterministically, so a future fix that removes the underlying
  issue surfaces an `unfulfilled_lint_expectations` warning rather
  than silently leaving a stale suppression.
- [`lint-downgrade-reason.md`](./lint-downgrade-reason.md) —
  same presence-and-length requirement as
  `perfectionist::lint_silence_reason`, extended to any
  `#[warn]` / `#[allow]` / `#[expect]` that lowers the lint's
  inherited level (`deny → warn`, `warn → allow`, etc.).
  Ancestry-aware counterpart.

### Clap derive help
- [`clap-help-no-markdown.md`](./clap-help-no-markdown.md) — forbid
  markdown constructs (HTML, links, intra-doc links, code blocks, code
  spans, headings) in doc comments that clap derive macros consume as
  help text. Disabled when the item carries an explicit help override
  (`#[arg(help = ...)]`, `#[clap(about = ...)]`, etc.).
- [`clap-help-length.md`](./clap-help-length.md) — flag clap-bound doc
  comments that exceed configurable line / character budgets (catches
  AI-generated bloat). Same override set of doc-comment containers.

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
- **"Use the most encompassing type" case** of the pacquet guide's
  owned-vs-borrowed section is already covered by
  `clippy::ptr_arg` (flags `&PathBuf` / `&String` / `&Vec<_>` and
  suggests `&Path` / `&str` / `&[_]`). The other two cases
  (prefer-owned-when-converting and
  prefer-borrowed-when-not-consumed) are handled respectively by
  [`prefer-owned-parameter`](./prefer-owned-parameter.md) and
  `clippy::needless_pass_by_value`.
- **Reporter wire-contract requirements** in pacquet (channel naming,
  upstream permalink comments, ordering relative to side effects, recording
  fakes in tests) — these depend on cross-repo invariants and human
  judgement about the upstream call site.
- **Pattern-match wrapping style** — the "concise wrapping style" example
  is already idiomatic and there is no concrete anti-pattern to detect.
- **Test logging guidance** (`assert_eq!` with multi-line strings should
  log via `eprintln!`, complex structures via `dbg!`) — the rule branches
  on the runtime *shape* of values, which a static lint cannot inspect.
- **Cloning `Arc` / `Rc` via `.clone()`** (pacquet *Cloning `Arc` and
  `Rc`*) — already covered by `clippy::clone_on_ref_ptr`, which flags
  `x.clone()` on a ref-counted pointer and suggests the qualified
  `Arc::clone(&x)` / `Rc::clone(&x)` form. It is a `restriction` lint
  (allow-by-default); a project that wants it enables the single line
  `clone_on_ref_ptr = "warn"` under `[lints.clippy]`. This was briefly
  implemented as `perfectionist::arc_rc_clone`, but a verbatim
  reimplementation of a stock Clippy lint earned no keep and forced an
  interop downgrade to avoid double-firing when both were on, so it was
  removed (KSXGitHub/parallel-disk-usage#422, pnpm/pnpm#11839).
- **Doc comments referencing items more private than the documented item**
  (pacquet *Documentation comments*) — already covered by rustdoc's
  built-in `rustdoc::private_intra_doc_links` lint (default `warn`).
  Run `RUSTFLAGS='-D warnings' cargo doc --document-private-items` to
  promote it to a hard error. The bare-backtick variant
  (`` `Foo` `` rather than `` [`Foo`] ``) is funnelled into intra-doc
  links by the [`intra-doc-links`](./intra-doc-links.md) rule, after
  which rustdoc catches it. Reimplementing this in Dylint would be a
  less accurate duplicate of rustdoc's own resolver.
