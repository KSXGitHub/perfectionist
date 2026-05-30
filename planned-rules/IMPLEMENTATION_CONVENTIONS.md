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
- `perfectionist::bare_issue_reference` (`src/rules/bare_issue_reference.rs`)
  — skips code regions, existing links, and reference-link
  definitions before flagging bare `#123` tokens.
- `perfectionist::bare_url` (`src/rules/bare_url.rs`) — skips code
  regions, autolinks (`<...>`), labelled links, and reference-link
  definitions before flagging bare `http(s)://` URLs.
- `perfectionist::unicode_ellipsis_in_docs`
  (`src/rules/unicode_ellipsis_in_docs.rs`) — strips code regions,
  then byte-scans for U+2026.
- [`em-dash-prose`](./em-dash-prose.md) — strips code regions, then
  byte-scans for `—` / `–`.

They share one crate-internal scanner at `src/markdown.rs`, built
from `take_*` combinators per the "Parser style" section above.
The bare-* family already populates Tier A of the surface
described below; the rules listed above as still-planned extend it
as they're implemented. The helper is hand-written. **Do not pull
in `pulldown_cmark`, `comrak`, `markdown-rs`, or `markdown-it`**
for any of these rules without first revisiting the rationale
below.

### Two tiers of consumer

Two needs sit on top of the same primitives.

- **Tier A — structural classification.** Distinguishes a code
  span from an inline link from a reference definition from an
  autolink from an HTML tag from a heading. Consumers:
  `intra_doc_links`, `clap_help_no_markdown`, `bare_issue_reference`,
  `bare_url`.
- **Tier B — code-region mask.** Only needs the predicate "is this
  byte inside a code span or code block?". Consumers:
  `perfectionist::unicode_ellipsis_in_docs` (implemented);
  `em_dash_prose` (planned). The mask is `take_code_span` plus
  `take_code_block` in a loop over the input — `src/markdown.rs`'s
  `scan_code_regions`, not a separate Tier-A-style classifier.

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

## Reaching every module (source-layout rules)

A rule that inspects the **source-level layout of items** — the
granularity of `use` trees (`perfectionist::import_granularity`,
`src/rules/import_granularity.rs`), their blank-line grouping
(`perfectionist::import_grouping`, `src/rules/import_grouping.rs`),
the `self`-in-`use` handling
(`perfectionist::self_import`, `src/rules/self_import.rs`), or
anything else that reads the *written* shape of a module body rather
than a semantic property — must reach **every module in the crate**,
including separate-file `mod foo;` submodules nested to any depth.

The obvious implementation is wrong, and has been written wrong
**twice** so far. Both times the rule shipped as a pre-expansion
`EarlyLintPass` that walked the AST module tree; both times it
silently linted only the crate-root file and inline `mod { ... }`
blocks, skipping every separate-file submodule; both times that was
caught only later and fixed by moving to a `LateLintPass`:

- `import_granularity` shipped buggy in
  [#153](https://github.com/KSXGitHub/perfectionist/pull/153), fixed
  in [#173](https://github.com/KSXGitHub/perfectionist/pull/173)
  (`parallel-disk-usage#431`).
- `import_grouping` shipped buggy in the first commits of
  [#174](https://github.com/KSXGitHub/perfectionist/pull/174) and was
  fixed within the same PR (commit `61c7f81`) — *even though that
  PR's own description named the trap*. Knowing about the bug was not
  enough to avoid writing it.

### Why the obvious version is wrong

A pre-expansion `EarlyLintPass` is the natural reach for a rule that
needs the raw, un-cfg-stripped AST (cfg-disabled code is gone after
macro expansion, and a layout rule wants to see what the author
wrote). But **pre-expansion, an out-of-line `mod foo;` is still
`ModKind::Unloaded`** — its file is not parsed until macro expansion
runs. The AST walk reaches the declaration but not the body, so every
separate-file submodule is invisible. The rule passes its
single-file UI fixtures and then misses most of any real crate.

### The required shape

Run as a **`LateLintPass`** and **re-parse the crate's module files**
through the shared `src/module_reparse.rs` helper. Re-parsing reaches
every file while keeping `#[cfg(...)]` gates intact (parsing does not
strip cfg — the property the pre-expansion pass was reaching for),
and the throwaway `ParseSess` shares the real `SourceMap`, so spans
and autofix suggestions still point at the real source. Do **not**
write a fresh module-discovery or re-parse path; route through:

1. **`module_reparse::parse_crate_module_files(cx)`** (or the thin
   `for_each_module_file` wrapper) to get each file's freshly parsed
   `Crate` plus `live_module_spans`. The set is already scoped to
   real on-disk files that back a module in the HIR tree, so
   `include!` fragments, `include_str!`-ed data, and
   proc-macro-synthesised modules are excluded.
2. **Guard descent into an inline `mod { ... }` with
   `live_module_spans`.** A re-parse keeps cfg-*disabled* inline
   modules (e.g. `#[cfg(test)] mod tests { ... }` in a non-test
   build), so a walk that recurses into every `ModKind::Loaded`
   body unconditionally lints code that is **not in the compiled
   crate** — and, having no HIR node, those findings anchor at the
   crate root and cannot be silenced by a local `#[allow]`.
   `import_grouping` is the reference implementation: it consults
   `live_module_spans` and descends only into live modules. (At the
   time of writing, `import_granularity` and `self_import` route
   through `for_each_module_file`, which drops `live_module_spans`,
   so they descend unconditionally — an apparent divergence found by
   code reading but **not** yet pinned by a cfg-disabled-inline-module
   test. Confirm with such a test before treating either as a model
   to copy or "fixing" them.)
3. **`enclosing_hir::find_enclosing_hir_ids`** to anchor each parked
   violation at its enclosing HIR node, emitting through
   `clippy_utils::diagnostics::span_lint_hir_and_then`, so a
   per-module / per-item `#[allow]` / `#[expect]` resolves. Anchor on
   the **first `use`'s own span**, never the merged/replacement span:
   an out-of-line `mod foo;` item's span lives in the *parent* file,
   so a span there would fall back to the crate root.

A comment-only or token-only scanner that does not need the parsed
AST (the `bare_url` / `bare_email` / `bare_issue_reference` /
`unicode_ellipsis_in_*` family) has the same "which files are really
the crate's modules?" question and answers it with the same helper's
`module_reparse::crate_module_files` (see `src/comment_walk.rs` and
[#179](https://github.com/KSXGitHub/perfectionist/issues/179)) — do
not re-derive the file set there either.

### The decision rule, in one line

If a rule reads the *written layout* of items across module scopes,
it is a `LateLintPass` driven by `src/module_reparse.rs`, not an
`EarlyLintPass` module walk. Neither `EarlyLintPass` mode fits: a
**pre-expansion** pass keeps `#[cfg]`-gated code but leaves
out-of-line `mod foo;` modules `ModKind::Unloaded` (so it skips every
separate-file submodule — the bug above), while a **post-expansion**
pass loads those modules but has already had `#[cfg]`-disabled code
stripped (so it can't see what the author wrote under a false cfg).
A layout rule needs both reach and cfg-preservation at once, which
only re-parsing in a late pass gives. So if you reach for a
pre-expansion pass and match `ModKind` to walk module bodies, stop —
that is the trap.

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
plugins that both register a lint named `qualified_paths`
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
style = "unqualified"
```

The actual `dylint.toml` reads:

```toml
[perfectionist::qualified_paths]
style = "unqualified"
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

## Rule activation model

Every rule registered by this plugin is declared at the `Warn`
lint level. Each rule documents its **default state**: whether
the rule's pass installs at all when the consumer runs
`cargo dylint` without overrides.

```rust
pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Active;
// or
pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Inactive;
```

The two states map to the consumer-visible behaviour as follows:

- **Active by default.** The pass installs unconditionally,
  and the lint emits warnings wherever its trigger predicate
  fires. The consumer suppresses individual sites with
  `#[allow(perfectionist::<rule>)]` and turns the rule off
  globally by listing it under `[perfectionist].disable` in
  `dylint.toml`.
- **Inactive by default.** The pass is not installed during a
  `cargo dylint` run; the lint emits nothing. The consumer turns
  the rule on globally by listing it under `[perfectionist].enable`
  in `dylint.toml`. The lint *declaration* still registers
  either way, so `#[expect/allow/deny(perfectionist::<rule>)]`
  attributes at user call sites continue to resolve regardless
  of which array the rule appears under.

Reserve `Inactive by default` for rules whose triggers are
known to false-positive in real codebases, advisory sub-checks
gated behind a "are you sure?" knob, or rules whose preferred
configuration genuinely varies per project to the point that
shipping a baseline policy would be presumptuous. Everything
else is `Active by default`.

### Mandatory configuration on opt-in rules

A handful of `Inactive by default` rules express a *direction*
with no neutral baseline — `core_or_std` (`prefer_core` vs.
`prefer_std`), `qualified_paths` (`unqualified` vs. `qualified`),
`self_import` (`forbid` vs. `combined`), and `serde_wrapper_style`
(`transparent` vs. `from_into`). These rules deliberately do **not**
offer a `preserve`/no-op `style` value. "I don't want this rule" is
already expressed by leaving it out of `[perfectionist].enable`, so
a do-nothing `style` would only duplicate that — and a no-op enum
variant that shadows the activation mechanism is exactly the
redundancy this convention forbids.

The consequence is that `style` is **mandatory whenever the rule is
enabled** and has no default value. The validation is scoped to
activation: a rule reads and validates its `style` only when it
appears in `[perfectionist].enable`. A rule that is *not* enabled
never reads its configuration block, so omitting `style` for a
disabled direction rule is harmless and does not fail the run. Only
an *enabled* rule with a missing or invalid `style` is a
configuration error.

Severity escalation (`Warn → Deny → Forbid`) is the consumer's
prerogative and lives entirely outside the planning file. The
lint's declared level stays `Warn` in `declare_tool_lint!`; a
project that wants a stricter level on a particular rule reaches
for `#![deny(perfectionist::<rule>)]` at the crate root, or
`DYLINT_RUSTFLAGS="-D perfectionist::<rule>"` for a CI-wide
escalation — the same mechanisms rustc already exposes for
clippy and rustdoc lints. The planning file does not document
which projects should escalate; that is project-side policy and
out of scope for the catalogue.

