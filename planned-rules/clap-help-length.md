# `clap_help_length`

**Source:** project convention. Sibling lint to the implemented
`perfectionist::clap_help_markdown`
([`src/rules/clap_help_markdown.rs`](../src/rules/clap_help_markdown.rs));
both share the same "is this a clap-derived item, and is the help text
overridden?" detection.

## Statement

When a doc comment is consumed by clap's derive macros as help text,
flag the comment if it exceeds a configurable length budget. The
escape hatch — same as the markdown rule — is an explicit help
override (`#[arg(help = "...")]`, `#[clap(about = "...")]`, etc.).

The default budgets are sized to catch AI-generated bloat without
tripping on legitimately rich CLI options:

- **First paragraph (`about`):** at most 1 line, at most 120 characters.
- **Whole comment (`long_about`):** at most 8 lines, at most 600
  characters.

A "line" here is a non-empty `///` continuation; doc-comment leading
markers and trailing whitespace are stripped before counting.

## Why restrict this?

This is a stylistic preference, not a correctness issue.
CLI help text serves the user at a terminal, not a docs.rs reader. A
flag that prints an essay is hostile in `--help` output:

```text
  --manifest <PATH>
          Path to the package manifest file. The manifest defines the
          package metadata, including the name, version, dependencies,
          and entry points. By convention this file is named
          `package.json` and lives at the root of the workspace. If the
          file is missing, pacquet will fall back to discovering one
          via …
```

That text was probably auto-completed in seconds and reads poorly. The
lint nudges the author either to trim the comment or to move the long
prose into `#[arg(long_help = "...")]` (or `#[clap(long_about = "...")]`
on the struct), keeping the doc comment available for `cargo doc`.

## What to lint

For every `///` doc comment attached to a clap-derived container or to
a field/variant of one (the same container set as
`perfectionist::clap_help_markdown`):

1. Strip leading `///` / `//!` markers and surrounding whitespace from
   each line.
2. Split on the first blank line. The portion before is the *first
   paragraph* (`about`); the entire stripped block is the *whole*
   comment (`long_about`).
3. Compute line count and character count for each.
4. Emit a separate diagnostic when either threshold is exceeded.

The lint **does not fire** when an override key is present
(`about` / `long_about` / `help` / `long_help`), the same set of
override keys as the markdown rule.

## Examples

**Avoid:** about exceeds the line budget

```rust
#[derive(clap::Parser)]
struct Cli {
    /// Walks the source tree starting at the directory specified by
    /// the user, ignoring hidden entries unless `--all` is given,
    /// and prints a tabulated summary of each file's size on disk.
    root: PathBuf,
}
```

**Prefer:** (trimmed)

```rust
#[derive(clap::Parser)]
struct Cli {
    /// Root directory to scan.
    root: PathBuf,
}
```

**Prefer:** (rich docs preserved, help overridden)

```rust
#[derive(clap::Parser)]
struct Cli {
    /// Walks the source tree starting at the directory specified by
    /// the user, ignoring hidden entries unless `--all` is given.
    /// See [`Walker`] for the underlying iterator.
    #[arg(help = "Root directory to scan.")]
    root: PathBuf,
}
```

## Implementation notes

- `LateLintPass`, sharing the clap-container detection approach with
  the implemented `perfectionist::clap_help_markdown`
  (`src/rules/clap_help_markdown/collect.rs`), which re-parses the
  crate's module files to recover the `#[derive(...)]` and override
  attributes that macro expansion has consumed by the late pass, and
  reaches every separate-file submodule. Factor that walk into a shared
  helper when implementing this rule.
- Doc-comment normalisation: stitch all `#[doc = "..."]` attribute
  values in source order, strip the leading marker the lexer already
  removed, retain blank-line separators, and trim trailing whitespace
  per line. `clippy_utils::source::snippet` for the original spans, or
  use the rendered string from `attr.value_str()`.
- Counting: graphemes vs. bytes vs. chars. Default to *Unicode
  scalar values* (Rust `char` count) — close enough for CLI text, and
  sidesteps the grapheme-cluster crate dependency.
- The two thresholds (`about` and `long_about`) emit distinct
  diagnostics so a project can tune them independently.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Configuration

```toml
[clap_help_length]
about_max_lines = 1
about_max_chars = 120
long_about_max_lines = 8
long_about_max_chars = 600

# Recognised overrides; same default as `perfectionist::clap_help_markdown`.
override_keys = ["about", "long_about", "help", "long_help"]

# Set true to count graphemes instead of `char`s. Brings in the
# `unicode-segmentation` crate; off by default.
count_graphemes = false
```

A project that wants a single budget can set the same value for all
four `*_max_*` knobs.

## Default state

Active by default.

## Autofix

Not mechanical — trimming requires editorial judgement — so the
lint emits a help-only suggestion pointing at
`#[arg(long_help = "...")]` as the canonical escape hatch.

## Interaction with `perfectionist::clap_help_markdown`

The two lints are independent:

- `perfectionist::clap_help_markdown` catches doc-comment markdown
  that leaks into `--help` output.
- `clap-help-length` catches sheer volume.

Both share the clap-container detection and the override-key set;
disabling one does not affect the other.
