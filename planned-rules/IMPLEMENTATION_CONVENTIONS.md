# Implementation conventions

Conventions shared across multiple rules in this catalogue. Each rule
is otherwise self-contained; this file exists so the recurring
guidance lives in exactly one place.

## Parser style

When a rule needs to parse a non-trivial string — a URL, an email
address, a markdown span, a serde-attribute type literal, a
`#[derive(...)]` list — **prefer parser combinators over regex-style
matching**. Implement the scanner as a collection of small `take_*`
functions, each one consuming a prefix of its input and returning the
remainder.

The canonical shapes are:

```rust
// Always succeeds. Used for parsers that consume zero or more bytes
// of a known shape (skipping whitespace, taking a run of digits when
// "no digits" is a valid result, etc.).
fn take_whitespace(input: &str) -> ((), &str);

// Optional match. Used when the parser may not apply at this
// position; `None` lets the caller try a different alternative.
fn take_url_scheme(input: &str) -> Option<(UrlScheme, &str)>;

// Fallible match. Used when the parser commits to consuming the
// input but the input may be malformed. The `Error` carries enough
// context for a useful diagnostic.
fn take_email_local_part(input: &str) -> Result<(Cow<'_, str>, &str), EmailParseError>;
```

The pattern is well-trodden — `nom`, `chumsky`, `combine`, and
`winnow` all expose the same essential shape. Within `perfectionist`
these helpers are implemented by hand to avoid pulling a parser
library through the Dylint pass; the resulting code is small,
inspectable, and free of regex backtracking surprises.

### Why this style

- **Composition is explicit.** A complex parser is a sequence of
  small calls, each returning the remainder. The reader follows the
  data flow without skimming a regex line for capture-group offsets.
- **Each step is testable in isolation.** Unit tests sit next to the
  individual `take_*` function, exercise its corner cases, and run
  in microseconds.
- **No regex dependency.** Dylint passes load into the compiler
  process; every transitive crate is paid for at lint time. A regex
  engine is a poor fit for the small, fixed grammars these rules
  match.
- **Trailing-punctuation handling falls out naturally.** The
  caller's choice of when to commit to a match is visible in the
  call graph rather than buried in a non-greedy quantifier.
- **Span construction stays accurate.** Each combinator knows
  exactly how many bytes it consumed; converting that into a `Span`
  on the source map is a one-liner per call site.

### What to avoid

- Pulling in `regex`, `regex_lite`, or `aho-corasick` for what is
  fundamentally a small lexer. These crates are excellent in their
  niche; that niche is not a Dylint pass with a five-line grammar.
- Cramming a multi-step parser into a single `chars().filter().fold()`
  chain. The fold loses intermediate state and obscures where the
  failure points are.
- Returning byte offsets without the slice. The combinator return
  type bundles `(Parsed, &str)` so the caller cannot accidentally
  forget to advance.

### Where to draw the line

Trivial scans — locating one Unicode codepoint, counting lines,
matching a literal three-byte UTF-8 sequence — do not need a
combinator scaffold. Reach for `&[u8]::iter()` or
`memchr::memchr` directly. The combinator style is for rules whose
grammar has more than one alternative, more than one position-
dependent decision, or any kind of optional / repeated structure.

The rules that explicitly call this convention out in their
implementation notes are the candidates; rules that just walk a
fixed-size byte sequence do not.

## Markdown parsing

Six rules in this catalogue scan a slice of markdown:

- [`intra-doc-links`](./intra-doc-links.md) — distinguishes
  `` `Foo` `` (candidate) from `` [`Foo`] ``, `[Foo]`,
  `[Foo](path)`, `[Foo][id]` (already linked).
- [`clap-help-no-markdown`](./clap-help-no-markdown.md) — classifies
  every banned construct (links, code spans, code blocks, HTML
  tags, headings, reference definitions) and emits a per-construct
  diagnostic.
- [`bare-issue-reference`](./bare-issue-reference.md) — skips code
  regions, existing links, and reference-link definitions before
  flagging bare `#123` tokens.
- [`bare-url`](./bare-url.md) — skips code regions, autolinks
  (`<...>`), labelled links, and reference-link definitions before
  flagging bare `http(s)://` URLs.
- [`unicode-ellipsis-in-docs`](./unicode-ellipsis-in-docs.md) —
  strips code regions, then byte-scans for U+2026.
- [`em-dash-prose`](./em-dash-prose.md) — strips code regions, then
  byte-scans for `—` / `–`.

They share one crate-internal scanner, to be added at
`src/markdown.rs`, built from `take_*` combinators per the
"Parser style" section above. The helper does not exist yet — the
first rule to need it implements it; subsequent rules consume it.
The helper is hand-written. **Do not pull in `pulldown_cmark`,
`comrak`, `markdown-rs`, or `markdown-it`** for any of these rules
without first revisiting the rationale below.

### Two tiers of consumer

Two needs sit on top of the same primitives.

- **Tier A — structural classification.** Distinguishes a code
  span from an inline link from a reference definition from an
  autolink from an HTML tag from a heading. Consumers:
  `intra_doc_links`, `clap_help_no_markdown`, `bare_issue_reference`,
  `bare_url`.
- **Tier B — code-region mask.** Only needs the predicate "is this
  byte inside a code span or code block?". Consumers:
  `unicode_ellipsis_in_docs`, `em_dash_prose`. The mask is
  `take_code_span` plus `take_code_block` in a loop over the input;
  no separate scanner.

### Combinator surface

One `take_*` per CommonMark construct the catalogue recognises:

- `take_code_span` — between matching `` ` `` runs of equal length.
- `take_code_block` — fenced (triple-backtick or `~~~`) or
  four-space indented.
- `take_link` — `[text](dest)`, `[text][id]`, `[text]`, `` [`Type`] ``.
- `take_autolink` — `<https://...>`, `<mailto:...>`.
- `take_reference_definition` — `[id]: dest` at block start.
- `take_html_tag` — `<tag ...>` and `</tag>`.
- `take_heading` — ATX (`# h`) and Setext (`h\n===`).

Each combinator returns the matched substring and the remainder
per the canonical shapes in "Parser style". Rust-specific
extraction layered on top — `intra_doc_links` pulling an
identifier out of a `take_code_span` result, `bare_url` pulling a
scheme out of `take_autolink` failure-fallback prose — lives in
each rule's own module, not in `src/markdown.rs`.

### Why hand-rolled rather than a library

A Dylint plugin loads into rustc's process; every transitive crate
is paid for at lint time. The grammar these six rules need is
seven constructs, no inline-emphasis precedence, no link-reference
resolution across the whole comment. No library hits that target
without overshooting:

- **`pulldown_cmark`** — the de facto Rust choice. Event-based,
  carries source offsets via `OffsetIter`, MIT, fast, used by
  `mdbook` and historically by rustdoc. For a seven-construct
  predicate it is still ~2-3k LoC of dependency loaded into
  rustc, and consumers must map its event taxonomy onto the
  lints' construct taxonomy. The closest fit, still not free.
- **`comrak`** — CommonMark + GFM, AST-based. Brings
  `typed-arena`, `unicode-categories`, `entities`, `slug`, `xdg`.
  Heavier than `pulldown_cmark` and aimed at GFM rendering, not
  at "give me byte spans of code regions".
- **`markdown-rs`** (`wooorm/markdown-rs`) — CommonMark + GFM +
  MDX + frontmatter. Most spec-faithful, largest dep tree, worst
  weight-vs-need ratio for this use case.
- **`markdown-it`** — JS port. Pluggable. Less battle-tested in
  Rust than `pulldown_cmark`.

The combinator approach also keeps span construction precise
without a mapping layer: each `take_*` knows exactly how many
bytes it consumed, which is how the lints' diagnostics anchor
into the source map.

### Rustdoc flavour

Rustdoc intra-doc links — `` [`Foo`] ``, `[Foo]`,
`[Foo](crate::foo::Foo)` — are *plain CommonMark* at the parser
level. What makes them intra-doc links is rustdoc's *post-parse*
resolution step, which tries each link's destination text as a
Rust path through the documented item's scope. No general-purpose
markdown library models that resolution; rustdoc's own pipeline
lives in `rustc_resolve` and `rustdoc::html::markdown` and is not
published as a library.

The practical consequence: the scanner's job is to say "this is a
link, here is its destination text". Whether the destination
resolves as a Rust path is `intra_doc_links`'s job, performed
against `TyCtxt` in a `LateLintPass`, not the scanner's.
Consumers that need only "is this any kind of link?" (e.g.,
`clap_help_no_markdown`, which rejects all link forms) stop at the
scanner's answer.

This also means library choice is downstream of the intra-doc-link
question, not upstream of it: even if a hypothetical library
parsed rustdoc-flavoured markdown, the resolution layer would
still be custom code in this repo.

### Where to revisit this decision

The decision is per-helper, not per-codebase. If, during
implementation of `clap_help_no_markdown`, the HTML-tag and
reference-definition combinators turn out to dominate the helper's
complexity — they cover constructs none of the other five rules
need — that single rule may switch to a vendored `pulldown_cmark`
walk while the other five continue on the hand-rolled helper.
Open a follow-up PR; do not silently expand `src/markdown.rs`'s
dependency surface for the other consumers.

## Lint name namespacing

Every lint registered by this plugin lives in the `perfectionist`
*tool namespace*. The planning files in this directory use the
unqualified form for readability — `qualified_paths` reads better
than `perfectionist::qualified_paths` in a sentence — but the lint
as it appears in `declare_tool_lint!`, in the `dylint.toml`
configuration table, in `#[allow(...)]` / `#[deny(...)]`
attributes, and in compiler diagnostic output is always
namespaced.

### Why namespace at all

Dylint loads each plugin as a separate dynamic library, but
rustc's `LintStore` is a single global table per compilation. Two
plugins that both register a lint named `single_letter_names`
cause rustc to reject the second registration as a duplicate. The
names this catalogue chose — `from`, `bare_url`, `qualified_paths`,
`serde_source_types`, and similar — are exactly the names an
independent plugin author would reach for, so collisions are not
hypothetical. Namespacing removes them.

The namespace also makes diagnostic attribution unambiguous. A
warning's note reads `#[warn(perfectionist::qualified_paths)] on
by default`, naming the source plugin so there is no question
which library to consult or configure.

### Why a tool namespace rather than a bare prefix

Two reasonable approaches exist:

- **Tool namespace** (`perfectionist::qualified_paths`): the
  approach used by `clippy::*` and `rustdoc::*`. Idiomatic, scoped,
  reads cleanly in `#[allow(...)]`.
- **Bare prefix** (`perfectionist_qualified_paths`): a single long
  identifier. Mechanically simpler, no tool registration required.

Both work for *this* plugin's compilation because the plugin is
already nightly (it depends on `rustc_private`) and can use
`rustc_session::declare_tool_lint!`, which threads through the
unstable `register_tool` machinery. During a `cargo dylint` run
the plugin's nightly toolchain compiles the consumer's code, the
tool name is registered, and `#[allow(perfectionist::foo)]` is
recognised exactly the way `#[allow(clippy::foo)]` is.

The tool-namespace form is preferred because it is the standard
Rust pattern for plugin-provided lints, separates the project's
namespace from the global lint name pool, and keeps user-visible
syntax close to the syntax users already know from clippy.

### Caveat: the consumer-side `unknown_lints` warning

The plugin can register `perfectionist` as a tool name during a
`cargo dylint` run, but it cannot do so during the consumer's
*normal* `cargo build` / `cargo check` (where the plugin is not
loaded). When stable rustc encounters `#[allow(perfectionist::foo)]`
in source without the plugin loaded, the behaviour depends on the
rustc version and is not perfectly stable across releases:

- Some rustc versions silently ignore unknown tool prefixes
  (treating them as "this attribute is for a tool I don't
  recognise; not my problem").
- Other versions emit an `unknown_lints` warning naming the
  unknown tool / lint.

Either behaviour is *also* what happens for the bare-prefix
alternative `#[allow(perfectionist_foo)]` — stable rustc has no
way to know either name belongs to a real lint. Tool namespace is
therefore at worst equivalent to bare prefix on this dimension,
and possibly *more* lenient because tool-prefixed unknowns are the
case rustc most often special-cases for cross-tool ergonomics.

The standard workaround for both forms is to add
`#![allow(unknown_lints)]` at the crate root of any project that
sees the warning. Document this in the project's user-facing
README so consumers know to apply it once if needed.

### How to apply the namespace

When a rule's planning file reads:

```text
# `qualified_paths`
```

the `declare_tool_lint!` invocation reads:

```rust
rustc_session::declare_tool_lint! {
    pub perfectionist::QUALIFIED_PATHS,
    Warn,
    "decide whether items from outside the current scope are named \
     by their full path or imported via `use`",
    report_in_external_macro: false
}
```

The macro produces a lint whose canonical printed name is
`perfectionist::qualified_paths`. The translation from planning
name to declaration is one-to-one: take the snake_case identifier
from the planning H1, uppercase it for the macro identifier, slot
it under `perfectionist::`. The diagnostic text inside the lint is
the rule's own one-line summary.

Configuration tables follow the same shape. The planning file
shows:

```toml
[qualified_paths]
style = "preserve"
```

The actual `dylint.toml` reads:

```toml
[perfectionist::qualified_paths]
style = "preserve"
```

A user-side suppression reads:

```rust
#[allow(perfectionist::qualified_paths)]
fn legacy_function() { /* ... */ }
```

A crate-root suppression of the cross-toolchain warning is:

```rust
#![allow(unknown_lints)]
```

