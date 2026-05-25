# `clap_help_no_markdown`

**Source:** project convention.

## Statement

When a doc comment (`///` or `//!`) is consumed by clap's derive macros
as help text, forbid markdown-specific constructs in that comment.
Specifically, disallow:

- HTML tags (`<br>`, `<code>`, `<a href="...">`, etc.).
- Inline links: `[text](https://example.com)`.
- Reference links: `[text][id]` and the trailing `[id]: ...` definitions.
- Intra-doc links: `` [`Type`] `` and `[Type]`.
- Code blocks (``` ``` ``` ```, indented four-space, or `~~~` fences).
- Code spans: `` `value` ``.
- Setext / ATX headings (`# Heading`, `Heading\n=====`).

Bold (`**text**`), italics (`*text*` or `_text_`), bullet lists, and
numbered lists are not flagged by default; clap renders them as their
literal characters in `--help`, which usually reads cleanly. They are
configurable.

## Why is this bad?

By default, clap does **not** render doc comments through a markdown
processor. The raw text is shown verbatim in the terminal `--help`
output. Writing `[`PathBuf`]` produces a docs.rs link in HTML output but
shows literally as `[`PathBuf`]` in the terminal — a classic two-audience
leak.

The escape hatch is to *override* the help text with a plain string,
keeping the rich doc comment for `cargo doc`:

```rust
/// Builds the lockfile by walking [`Dependency`] graphs.
#[arg(help = "Builds the lockfile by walking dependency graphs.")]
pub deps: PathBuf,
```

Once an override is present, the doc comment is no longer the source of
truth for help text and the lint stays silent.

## What to lint

For every `///` doc comment attached to:

- a struct that derives `clap::Parser`, `clap::Args`, `clap::Subcommand`,
  or `clap::CommandFactory`,
- a field of such a struct,
- an enum that derives `clap::Subcommand` or `clap::ValueEnum`,
- a variant of such an enum,

scan the rendered comment text for each banned construct and emit a
diagnostic at the construct's span.

The lint **does not fire** when the same item carries any of:

- `#[clap(about = "...")]`
- `#[clap(long_about = "...")]`
- `#[arg(help = "...")]`
- `#[arg(long_help = "...")]`
- `#[command(about = "...")]`
- `#[command(long_about = "...")]`
- `#[clap(verbatim_doc_comment)]` or `#[command(verbatim_doc_comment)]`
  — the user has opted into rendering the doc comment exactly as
  written, so flagging is no longer the right action; the lint instead
  emits a softer warning that markdown will leak into `--help`.

## Examples

```rust
// Bad
#[derive(clap::Parser)]
struct Cli {
    /// Path to the [`PackageManifest`].
    ///
    /// See [the manifest format](https://example.com/manifest).
    manifest: PathBuf,
}

// Good (override the help text with plain prose)
#[derive(clap::Parser)]
struct Cli {
    /// Path to the [`PackageManifest`].
    ///
    /// See [the manifest format](https://example.com/manifest).
    #[arg(help = "Path to the package manifest.")]
    manifest: PathBuf,
}

// Also good (no markdown in the first place)
#[derive(clap::Parser)]
struct Cli {
    /// Path to the package manifest.
    manifest: PathBuf,
}
```

## Implementation notes

- `LateLintPass`. Two halves:
  1. **Identify clap-derived containers.** Cache a `HashSet<DefId>` of
     local items whose attributes include `#[derive(...)]` mentioning
     a clap derive. Recognise the derive by path: `clap::Parser`,
     `clap::Args`, `clap::Subcommand`, `clap::ValueEnum`, plus the
     legacy `clap::Clap` for projects on older versions. Resolve the
     derive macro's `DefId` so re-exports (`pub use clap::Parser;`)
     are caught.
  2. **For each documented item, check membership.** A field's
     container is its struct; an enum variant's container is its enum.
     The lint fires only when the container is in the cached set.
- Share the "stitch `#[doc = ...]` attributes, walk, emit
  per-construct sub-spans" pipeline with
  [`intra-doc-links`](./intra-doc-links.md) and the implemented
  `perfectionist::unicode_ellipsis_in_docs`
  (`src/rules/unicode_ellipsis_in_docs.rs`), whose doc-comment block
  walking and span mapping already live in the crate-internal
  `src/comment_walk.rs`. Reuse that helper rather than re-deriving it.
- Use the shared markdown scanner (Tier A — structural
  classification) per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#markdown-parsing).
  Each banned construct in `forbid` maps to one `take_*` result;
  the dispatcher branches on the match and emits the right
  per-construct diagnostic. This rule is the most demanding
  consumer of the scanner — if its HTML-tag and
  reference-definition needs come to dominate, the convention's
  escape hatch (vendor `pulldown_cmark` for this rule alone)
  applies here.
- Override detection: walk attribute lists for `clap`, `arg`,
  `command` paths. Recognise `MetaNameValue` shape with the override
  key. `clippy_utils::attrs::find_by_name` is a starting point but
  needs path matching, not just symbol matching.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Configuration

```toml
[clap_help_no_markdown]
# Constructs to flag; default is the conservative set above.
forbid = ["html", "inline_link", "reference_link", "intra_doc_link",
          "code_block", "code_span", "heading"]

# Additional constructs a project may want to ban.
extra_forbid = ["bold", "italic", "list"]   # empty by default

# Recognise these attribute keys as overrides that disable the lint.
override_keys = ["about", "long_about", "help", "long_help"]
```

## Default state

Active by default.

## Autofix

Offered only for the trivial code-span case (`` `Foo` `` → `Foo`);
the other constructs depend on what the author intended and are
emitted as help-only suggestions.
