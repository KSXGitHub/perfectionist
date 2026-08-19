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

Several rules scan a slice of markdown: every rule that imports
`crate::markdown`. They share that one crate-internal scanner, built
from `take_*` combinators per [Parser style](#parser-style). The
helper is hand-written. **Do not pull in `pulldown_cmark`, `comrak`,
`markdown-rs`, or `markdown-it`** for any of them without first
revisiting the rationale below.

### Tiers of consumer

Consumers divide by how much structure they need.

- **Tier A — structural classification.** Distinguishes a code
  span from an inline link from a reference definition from an
  autolink from an HTML tag from a heading. Entry points:
  `scan_skip_regions` returns skip ranges to post-filter against,
  `classify_constructs` returns every construct's range *and* kind,
  and `scan_code_span_candidates` returns code spans alone.
- **Tier B — code-region mask.** Only needs the predicate "is this
  byte inside a code span or code block?". Entry point:
  `scan_code_regions` — `take_code_span` plus `take_code_block` in
  a loop over the input, not a separate Tier-A-style classifier.

### Combinator surface

One `take_*` per CommonMark construct the catalogue recognises:

- `take_code_span` — between matching `` ` `` runs of equal length.
- `take_code_block` — fenced (triple-backtick or `~~~`) or
  four-space indented.
- `take_link` — `[text](dest)`, `[text][id]`, `[text]`, `` [`Type`] ``.
- `take_autolink` — `<https://...>`, `<mailto:...>`.
- `take_reference_definition` — `[id]: dest` at block start.
- `take_html_tag` — `<tag ...>`, `</tag>`, comments, declarations.
- `take_atx_heading` / `detect_setext_headings` — ATX (`# h`) and
  Setext (`h\n===`) headings.
- `take_emphasis` / `take_list_marker` — `**bold**` / `*italic*` and
  bullet / ordered list markers, matched only when a consumer opts
  in via `ClassifyOptions`.

The full Tier A classifier `classify_constructs` stitches these into
one walk that returns each construct's byte range and kind. Each
combinator returns the matched substring and the remainder per the
canonical shapes in [Parser style](#parser-style). Rust-specific
extraction layered on top — pulling an identifier out of a
`take_code_span` result, pulling a scheme out of `take_autolink`
failure-fallback prose — lives in each rule's own module, not in
`src/markdown.rs`.

### Why hand-rolled rather than a library

A Dylint plugin loads into rustc's process; every transitive crate
is paid for at lint time. The grammar these rules need is a fixed
set of constructs (the only emphasis handling is a pragmatic,
opt-in `**bold**` / `*italic*` matcher, not full CommonMark
flanking precedence) and no link-reference resolution across the
whole comment. No library hits that target without overshooting:

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
resolves as a Rust path is the consuming rule's job, performed
against `TyCtxt` in a `LateLintPass`, not the scanner's.
Consumers that need only "is this any kind of link?" — a rule that
rejects all link forms, say — stop at the scanner's answer.

This also means library choice is downstream of the intra-doc-link
question, not upstream of it: even if a hypothetical library
parsed rustdoc-flavoured markdown, the resolution layer would
still be custom code in this repo.

### Where to revisit this decision

The decision is per-helper, not per-codebase.
`perfectionist::clap_help_markdown` — the most demanding consumer —
has since been implemented on the hand-rolled scanner: its HTML-tag,
heading, emphasis, and list combinators were added to `src/markdown.rs`
(see `classify_constructs`) without dominating the helper's complexity
or forcing a library, so no switch was needed. Should a future
construct tip that balance for one rule, that rule may switch to a
vendored `pulldown_cmark` walk while the others continue on the
hand-rolled helper. Open a follow-up PR; do not silently expand
`src/markdown.rs`'s dependency surface for the other consumers.

## Reaching every module (source-layout rules)

A rule that inspects the **source-level layout of items** — the
granularity of `use` trees (`perfectionist::import_granularity_mismatch`,
`src/rules/import_granularity_mismatch.rs`), their blank-line grouping
(`perfectionist::import_grouping_mismatch`, `src/rules/import_grouping_mismatch.rs`),
the module-`self` import folding
(`perfectionist::uncombined_self_import`,
`src/rules/uncombined_self_import.rs`), or
anything else that reads the *written* shape of a module body rather
than a semantic property — must reach **every module in the crate**,
including separate-file `mod foo;` submodules nested to any depth.

The obvious implementation is wrong, and has been written wrong
**twice** so far. Both times the rule shipped as a pre-expansion
`EarlyLintPass` that walked the AST module tree; both times it
silently linted only the crate-root file and inline `mod { ... }`
blocks, skipping every separate-file submodule; both times that was
caught only later and fixed by moving to a `LateLintPass`:

- `import_granularity_mismatch` shipped buggy in
  [#153](https://github.com/KSXGitHub/perfectionist/pull/153), fixed
  in [#173](https://github.com/KSXGitHub/perfectionist/pull/173)
  (`parallel-disk-usage#431`).
- `import_grouping_mismatch` shipped buggy in the first commits of
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
   `import_grouping_mismatch` is the reference implementation: it consults
   `live_module_spans` and descends only into live modules. (At the
   time of writing, `import_granularity_mismatch` and `uncombined_self_import` route
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
AST (the prose-scanning family: whichever rules import
`crate::comment_walk`) has the same "which files are really the
crate's modules?" question and answers it with the same helper's
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

## Naming a lint after the anti-pattern

A lint's name is read in `#[allow(...)]`, `#[expect(...)]`,
`#[warn(...)]`, `#[deny(...)]`, and `#[forbid(...)]`, so it must read
correctly in all five. The governing rule: **name the anti-pattern the
lint fires on, never the fix, the remedy, or the stylistic preference.**
`#[deny(perfectionist::<name>)]` should read as "forbid the bad thing";
`#[allow(perfectionist::<name>)]` as "permit the bad thing here".

A name that describes the desired *state* or the *recommended remedy*
inverts under these attributes and reads as nonsense:

- `derive_ordering` — `#[deny(... derive_ordering)]` reads "forbid
  ordering the derives", the opposite of intent. Named for the
  anti-pattern: `unordered_derives`.
- `prefer_derive_more_over_thiserror` — does not "forbid preferring
  `derive_more`"; it forbids `thiserror`. Named for the anti-pattern:
  `thiserror_usage`.
- `non_exhaustive_error` — flags error enums that *lack*
  `#[non_exhaustive]`, so the name states the opposite of the trigger.
  Named for the anti-pattern: `exhaustive_error_enums`.
- `clap_help_no_markdown`, `prefer_expect_over_allow`,
  `prefer_raw_string`, `prefer_owned_parameter`, `print_macro_split`,
  `inline_test_footprint`, `macro_argument_binding` — all named for the
  remedy, the preference, or a neutral topic rather than the violation.

This catalogue's first pass over its own names is recorded in
[#268](https://github.com/KSXGitHub/perfectionist/issues/268).

### Follow Clippy's naming idiom

Clippy lint names are short noun phrases or adjective-quantifier forms
naming the offending construct — `needless_*`, `redundant_*`,
`unused_*`, `excessive_*`, `too_many_*`, `large_*`, `exhaustive_*` —
not gerunds (`splitting_*`), not exhortations (`prefer_*`, `use_*`), and
not negations of a virtue (`*_no_*`, `non_*`). Reach for the same shape:

- A length / count cap uses an adjective quantifier, not a verb:
  `excessive_inline_tests`, not `inline_test_exceeds_footprint`. Clippy
  has `too_many_lines`, `excessive_nesting`, `large_enum_variant`; it
  has no `*_exceeds_*` / `*_exceeding_*` lint.
- A configurable-style rule names the *mismatch*, because the offending
  shape depends on the configured target style — there is no single
  fixed bad shape to name. `import_granularity_mismatch`,
  `import_grouping_mismatch`.

### Mirror the Clippy name only for a genuine refinement

When a perfectionist rule is a **refinement of an existing Clippy
lint** — the same anti-pattern, with a narrower trigger, a stricter
threshold, or extra configuration — give it the **same name** as its
Clippy counterpart (under the `perfectionist::` namespace). A reader who
knows the Clippy lint then transfers that knowledge directly.

- `allow_attributes_without_reason` mirrors
  `clippy::allow_attributes_without_reason` (adds a `min_reason_length`
  quality floor and an `exempt_lints` list).
- `allow_attributes` mirrors `clippy::allow_attributes` (rewrites
  `#[allow]` to `#[expect]`, restricted to deterministically-firing
  lints).
- `exhaustive_error_enums` echoes `clippy::exhaustive_enums` /
  `clippy::exhaustive_structs`, scoped to error-shaped types.

**The danger is mistaking a contradiction or a complement for a
refinement.** Mirror the name *only* when the perfectionist rule fires
on the same anti-pattern as the Clippy lint. Do **not** borrow the name
when the rule:

- flags the **opposite** direction (e.g. a rule that wants a parameter
  taken *by value* must not borrow the name of a Clippy lint that wants
  it taken *by reference* — `needless_borrowed_parameters` deliberately
  does not reuse `clippy::needless_pass_by_value`, which covers the
  reverse), or
- addresses an **orthogonal** concern that merely touches the same
  syntax (sharing a construct is not sharing an anti-pattern).

Borrowing a Clippy name for a rule that contradicts or complements it
would tell the reader the exact wrong thing about what the rule does.
When in doubt, the rule is *not* a refinement: give it its own
anti-pattern name rather than an inherited one.

### Do not over-claim in the name

The name may assert no more than the trigger actually checks. A rule
that flags a *trailing comment on a lint-level attribute* cannot tell
whether that comment is a suppression rationale, so it is
`lint_attribute_trailing_comment`, not `lint_reason_from_comment` — the
latter claims a "reason" the rule never verifies, and would also hide
that the rule covers `warn` / `deny` / `forbid`, not just the
`allow` / `expect` pair that the `allow_attributes*` family is about.

## Lint name namespacing

Every lint registered by this plugin lives in the `perfectionist`
*tool namespace*. The planning files in this directory use the
unqualified form for readability — `path_qualification_mismatch` reads better
than `perfectionist::path_qualification_mismatch` in a sentence — but the lint
as it appears in `declare_tool_lint!`, in the `dylint.toml`
configuration table, in `#[allow(...)]` / `#[deny(...)]`
attributes, and in compiler diagnostic output is always
namespaced.

### Why namespace at all

Dylint loads each plugin as a separate dynamic library, but
rustc's `LintStore` is a single global table per compilation. Two
plugins that both register a lint named `path_qualification_mismatch`
cause rustc to reject the second registration as a duplicate. The
names this catalogue chose — `from`, `bare_url`, `path_qualification_mismatch`,
`serde_source_types`, and similar — are exactly the names an
independent plugin author would reach for, so collisions are not
hypothetical. Namespacing removes them.

The namespace also makes diagnostic attribution unambiguous. A
warning's note reads `#[warn(perfectionist::path_qualification_mismatch)] on
by default`, naming the source plugin so there is no question
which library to consult or configure.

### Why a tool namespace rather than a bare prefix

These approaches are both reasonable:

- **Tool namespace** (`perfectionist::path_qualification_mismatch`): the
  approach used by `clippy::*` and `rustdoc::*`. Idiomatic, scoped,
  reads cleanly in `#[allow(...)]`.
- **Bare prefix** (`perfectionist_path_qualification_mismatch`): a single long
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
# `path_qualification_mismatch`
```

the `declare_tool_lint!` invocation reads:

```rust
rustc_session::declare_tool_lint! {
    pub perfectionist::PATH_QUALIFICATION_MISMATCH,
    Warn,
    "decide whether items from outside the current scope are named \
     by their full path or imported via `use`",
    report_in_external_macro: false
}
```

The macro produces a lint whose canonical printed name is
`perfectionist::path_qualification_mismatch`. The translation from planning
name to declaration is one-to-one: take the snake_case identifier
from the planning H1, uppercase it for the macro identifier, slot
it under `perfectionist::`. The diagnostic text inside the lint is
the rule's own one-line summary.

Configuration tables follow the same shape. The planning file
shows:

```toml
[path_qualification_mismatch]
style = "unqualified"
```

The actual `dylint.toml` reads:

```toml
["perfectionist::path_qualification_mismatch"]
style = "unqualified"
```

A user-side suppression reads:

```rust
#[allow(perfectionist::path_qualification_mismatch)]
fn legacy_function() { /* ... */ }
```

A crate-root suppression of the cross-toolchain warning is:

```rust
#![allow(unknown_lints)]
```

## Path-shaped config values: the leading-`::` convention

Several rules take configuration values that are Rust paths —
`foo::bar`-shaped strings matched against a `use` path, a
macro-invocation path, and the like. The **form** of such a value
signals whether it is meant to be an *absolute* path or a
*relative* (suffix / tail) one:

- An **absolute** path entry carries a leading `::` —
  `"::foo::bar"`.
- A **relative** entry (matched by suffix / tail) has no leading
  `::` — `"foo::bar"`.
- A field that accepts **both** reads the presence or absence of
  the leading `::` on each entry to decide whether that entry is
  absolute or relative.

The leading `::` mirrors Rust's own absolute-path syntax, so the
config reads the way the equivalent path would be written in
source.

### Caveat: keyword-rooted paths (`crate` / `self` / `super` / `Self`)

The "leading `::` ⇒ absolute" rule applies to **extern-crate**
paths only, because that is the one absolute form Rust spells with
a leading `::`. A path rooted at a path keyword is the exception:

- A **crate-root absolute** path is written `crate::foo`, with
  **no** leading `::`. `::crate` is not valid Rust syntax, so an
  absolute entry into the local crate must be spelled
  `"crate::..."`, never `"::crate::..."`.
- `self::` / `super::` are relative roots; they too are written
  without a leading `::`, and `::self` / `::super` are not valid
  syntax either.

So a field that wants absolute paths accepts two well-formed
shapes — `"::<extern crate>::..."` and `"crate::..."` — and must
**reject** the impossible `::`-led keyword forms (`"::crate"`,
`"::self"`, `"::super"`) rather than silently accepting them.
Treating `"::crate::internals"` as valid is a real bug, not a
harmless typo: it cannot occur in source, yet a naive "prepend
`::` to the import path" matcher will happily match it against
`use crate::internals::*;`. The typo'd entry then matches while the
correctly-spelled one (`"crate::internals"`) never does.

### Where it's implemented

- **`allowed_paths`** on `perfectionist::wildcard_imports`
  (`src/rules/wildcard_imports.rs`) and
  `perfectionist::named_prelude_imports`
  (`src/rules/named_prelude_imports.rs`) matches the whole
  (absolute) module path of an import by exact string equality, so
  every entry is absolute. The shared `src/abs_path.rs` helper
  carries both halves of the convention: `canonical_key` keys a
  crate-root path by its own spelling and an extern path with a
  leading `::` (so `"::rayon::iter"` matches both
  `use rayon::iter::*;` and `use ::rayon::iter::*;`), and
  `validate_absolute` rejects the impossible / relative entries
  (`::crate`, `::self`, `::super`, bare `self::` / `super::`, and a
  bare extern path missing its `::`) with a startup error.
- **The macro-path consumers** — `perfectionist::impure_macro_arguments`,
  `perfectionist::macro_trailing_comma`, and
  `perfectionist::overly_long_print_macro` — match a macro-invocation path
  against configured name lists (`extra_macros`, `allow_extra`,
  `deny_extra`, `ignore`, `target_macros`, …) through
  `src/macro_path.rs`. That matcher is deliberately
  **relative / tail-based**: a single-segment entry matches the
  invocation's final segment, and a multi-segment entry tail-matches
  the segment sequence. The no-`::` form is therefore the correct one
  for these entries. Absolute (anchored) macro-path matching is not
  implemented, so a `::`-led entry — which `parse_path` would
  otherwise silently coerce into a relative match by dropping the
  empty leading segment — is **rejected** at config-load time by
  `macro_path::reject_absolute`. Silently coercing absolute to
  relative is exactly the surprise this convention exists to
  prevent.

### Not subject to the convention

- `perfectionist::unpinned_repo_ref`'s `hosts` / `skip_hosts` /
  `hostname` are forge hostnames, not Rust module paths.

## Config shape: boolean fields, not an array of toggles

A fixed, small set of *independent* on/off switches is one boolean
field per switch — never a multi-select array backed by a
`Vec<enum>`. The tell-tale signs the array is wrong: it gets
flattened into N booleans before use, an empty array is overloaded
to mean "none", and duplicates are meaningless. Separate `bool`s say
exactly that and drop the empty-vs-absent ambiguity. Settled in
<https://github.com/KSXGitHub/perfectionist/pull/255> (which replaced
`wildcard_imports`' `exceptions = ["prelude", "root_reexport"]` with
the booleans `prelude_exception` / `root_reexport_exception`).

Some shapes are *not* this anti-pattern and stay as arrays/enums: a
single mutually-exclusive **choice** (a `style` / direction enum); an
**open-ended list** of user strings (`allowed_paths`, `extra_*`,
`ignore`); and a **permutation** where the sequence is itself the
configuration (an `order` field). The dividing line is
fixed-membership-and-independence, not length — so a borderline case
may keep the array as a *deliberate* call, argued for and documented
in the rule's own file rather than assumed here.

### Scan-surface toggles

The recurring concrete instance: a text-scanning rule's *where do I
scan?* surfaces are independent booleans, not a `targets` array. Reuse
the same field names so the config reads identically across rules:

```rust
/// Scan doc comments (`///`, `//!`, `/** */`, `/*! */`).
/// Defaults to `true`.
scan_doc_comments: bool,
/// Scan regular comments (`//`, `/* */`). Defaults to `true`.
scan_regular_comments: bool,
/// Scan string literals (`"..."`, `r"..."`).
scan_string_literals: bool,
```

The field names are the convention; each default is a per-rule call.
The comment surfaces default `true`; a rule may default
`scan_string_literals` off where a literal URL is program data rather
than prose (`perfectionist::unpinned_repo_ref` does).

`perfectionist::bare_email` and `perfectionist::unpinned_repo_ref`
already follow the field-name convention; a rule scanning only a
subset omits the surfaces it can't reach rather than renaming the ones
it keeps.

## Suppressing proc-macro-synthesised violations

`declare_tool_lint! { ... report_in_external_macro: false }` is the
flag every rule reaches for to avoid firing on code the user did not
write. It is necessary but **not sufficient**: rustc applies that
filter to the *diagnostic (primary) span* alone, and a whole class of
proc-macro expansions defeats it. This section records the failure mode
and the prescribed guards, because the gap has been rediscovered the
hard way on more than one rule. (The late-pass helper's own mechanics
are also documented at its definition in `src/common.rs`; this is the
audience-facing version for rule authors.)

### The failure mode

Derive macros such as `clap_derive` synthesise statements, parameters,
and attributes that exist only in the expansion — but they deliberately
stamp the *key token* of each synthesised node (the binding identifier,
the method-call segment, the `allow` in a generated `#[allow(...)]`)
with the **user-source span** of the attribute that drove the
expansion (`#[clap(default_value_t)]`, `#[clap(long)]`). The intent is
ergonomic: if the generated code later fails to compile, the error
points at an attribute the user can actually edit rather than at
invisible expander output.

The side effect is that the node's diagnostic span carries the *root*
syntax context and resolves to real source. `report_in_external_macro:
false` sees a user-authored span and lets the lint through, so the rule
fires on a `#[clap(...)]` field — or a `default_value_t` binding — that
the user cannot rename or annotate. A false positive on unfixable code.

**A rule is vulnerable exactly when its diagnostic span is narrower
than the syntactic node that produced the violation** — an identifier,
a method segment, an attribute name. A rule whose primary span covers
the whole offending construct is already handled by the built-in filter
and needs nothing extra. Make this test the moment you choose the
diagnostic span for a new rule.

### The fix, keyed by pass kind

The synthesised node *is* reachable as external-macro output; you just
have to look past the diagnostic span. How you look depends on what the
pass has in hand, which is why the two historical fixes took different
routes:

- **`LateLintPass`** — call
  `crate::common::hir_in_external_macro(cx, hir_id, span)`. It checks
  the node's own span *and* the enclosing item's `def_span`. The second
  check is load-bearing for spans that are nothing but the identifier
  (a `<T>` generic parameter has no surrounding tokens to carry the
  expansion's `SyntaxContext`); the synthesised owner item's `def_span`
  does carry it.
- **`EarlyLintPass`** (and the pre-expansion half of a split rule,
  before any HIR or `def_span` exists) — call
  `clippy_utils::is_from_proc_macro(cx, node)`. It re-reads the source
  text under the node's span and compares it against the text the node
  *claims* to be; a generated `#[allow(...)]` whose underlying source
  reads `#[clap(...)]` fails the comparison and is skipped.
  `allow_attributes_without_reason` and `allow_attributes` apply it this
  way.

Reach for the variant your pass supports. Do not try to reproduce
`hir_in_external_macro`'s `def_span` walk in an early pass — there is no
HIR yet — and do not fall back to a bare `span.from_expansion()` /
`span.in_external_macro()` on the diagnostic span, which is exactly the
check this section exists to warn you is insufficient.

### The regression fixture must actually exercise the guard

A rule that the "vulnerable exactly when" test cleared needs nothing
here — skip this subsection. For a rule that *is* vulnerable and now
carries a guard, the guard is invisible in an ordinary UI test:
hand-written source never produces the pathological span. Add a
`ui/<rule>_proc_macro.rs` fixture that applies a derive from
`ui/auxiliary/proc_macro_synth_binding.rs` reproducing the
`clap_derive` span shape on your rule's node kind, and assert an empty
`.stderr`.

The trap — and it has already cost one round of work — is that the
fixture is only meaningful if its synthesised trigger is one the rule
**would otherwise fire on**. A fixture built around a node the rule
exempts or treats as trivial passes whether or not the guard exists: it
exercises nothing and bestows false confidence. Concretely:

- `allow_attributes` exempts `#[allow(dead_code)]`, so a fixture
  using the `dead_code`-emitting `SynthSilenceReason` derive is vacuous.
  It needs `SynthAllowRewriteable`, which emits a *rewriteable*
  `#[allow(non_snake_case)]`.
- `single_letter_closure_param` exempts trivial closures, so its
  `SynthClosure` derive emits a deliberately non-trivial body.

Two checks make non-vacuity concrete: (1) pick (or add) a derive whose
synthesised node is one a hand-written equivalent *would* be flagged
for — not an exempt or trivial shape; and (2) **mutation-check the
fixture** — temporarily delete the guard, confirm the fixture turns
red, then restore it. If removing the guard leaves the `.stderr` empty,
the fixture protects nothing.

The aux crate already exposes derives for the common node shapes
(`SynthBinding`, `SynthFnParam`, `SynthGeneric`, `SynthClosure`,
`SynthAllowRewriteable`, …); add a new one only when no existing derive
emits a *non-exempt* node of the kind your rule triggers on. A fixture
that passes both checks is what stops a later refactor from silently
regressing the suppression.

### Deliberate non-participants

Some rules skip all of the above on purpose:

- A rule declared `report_in_external_macro: true` *wants* to fire
  inside macro output; the guard would defeat its purpose. The `true`
  flag is itself the visible record of that intent, and grepping for
  it is how you find every such rule.
- A rule whose trigger cannot realistically be derive-generated may
  forgo the guard. `exhaustive_error_enums` was excluded on this basis —
  it is off by default and its `pub` error-shaped trigger is an
  unlikely derive output — though that reasoning currently lives only
  in the PR that made the call, not in the rule's source. Prefer to
  record such a decision where the next implementer will look: a short
  comment at the rule's span-selection site, so the omission reads as
  deliberate rather than forgotten.

### History

The class has surfaced in two shapes — a missing guard in production
code, and a guard present but never actually tested:

- `single_letter_let_binding` false-positived on `default_value_t`
  bindings; patched inline first, then generalised into
  `hir_in_external_macro` and applied across the sibling late rules.
- `allow_attributes_without_reason` false-positived on `clap_derive`'s generated
  `#[allow(...)]`; fixed with the early-pass `is_from_proc_macro`
  variant, since the late-pass helper did not apply.
- `allow_attributes` shipped *with* the early-pass guard in
  place — it learned from `allow_attributes_without_reason` — but its first
  regression fixture reused the `dead_code` derive the rule exempts
  anyway. The test was vacuous: it would have passed even with the
  guard deleted. A follow-up commit added a rewriteable-`#[allow]`
  derive (`SynthAllowRewriteable`) that genuinely exercises the guard.

Two lessons, then. The first two say: apply the "vulnerable exactly
when" test when you pick a diagnostic span, and add the pass-keyed
guard. The third says: a regression fixture for this class is itself
easy to get wrong — confirm it fails with the guard removed, or it is
guarding nothing. If this section feels irrelevant to the rule you are
adding, re-run both checks before moving on; that is the step every
past regression skipped.

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
with no neutral baseline — `path_qualification_mismatch` (`unqualified` vs.
`qualified`), `serde_wrapper_form_mismatch` (`transparent` vs.
`from_into`), and `import_grouping_mismatch` (`single_block` vs.
`multi_block`). These rules deliberately do **not**
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

## `declare_tool_lint!` docs describe behaviour, not pass machinery

`tools/gen-docs/` renders a rule's `declare_tool_lint!` rustdoc
verbatim into the in-tree catalogue (`rules/<rule>.md`) and the docs
site, so that block must describe *what* the rule flags and *why* —
never *how* the pass is implemented. "Late pass", "pre-expansion",
"lowering", "name resolution", "HIR node", the queue/anchor mechanism,
and the like mean nothing to a catalogue reader. When a
user-observable limitation *stems* from the implementation, state the
limitation behaviourally (e.g. "resolved crate-wide rather than
per-module") and drop the mechanism that causes it ("the pass runs
pre-expansion and does not consult name resolution").

The convention is scoped to the `declare_tool_lint!` block. Docs on
*internal* items (queue structs, `register_pass`/`register_lint`,
`emit` helpers, source walkers) describe implementation freely — they
never reach users. And a doc may name a *rustc* mechanism the user
observes directly (`unfulfilled_lint_expectations` notes,
`unknown_lints`): that is behaviour the consumer sees, not this
plugin's pass internals.

## GitHub-specific markdown in rule docs

A rule's `declare_tool_lint!` rustdoc is rendered by rustdoc and by
`tools/gen-docs/` as well as by GitHub, so it must stay within the
markdown all of them understand; the planning files in this directory
carry no such constraint. Which renderer drops what, and when a `>`
block may become an alert, are settled in
[GitHub-specific markdown](../CLAUDE.md#github-specific-markdown).

