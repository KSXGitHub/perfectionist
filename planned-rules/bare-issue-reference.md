# `bare_issue_reference`

**Source:** project convention.

## Statement

In doc comments (`///`, `//!`), references to issues or pull
requests written as the bare token `#<digits>` must be rendered
as markdown links. Two acceptable forms:

- Inline: `[#123](https://github.com/owner/repo/issues/123)`.
- Reference: `[#123]` paired with `[#123]: https://github.com/owner/repo/issues/123`.

By default the rule applies only to *doc comments*. Plain `//`
comments are not processed as markdown, so the standard
`[#N](url)` suggestion would appear there as literal syntax
rather than a link — there is no useful fix to offer in
labelled-link form. Opt in via `include_plain_comments` to lint
plain comments too; the autofix in that mode substitutes the URL
itself (bare, or wrapped in angle brackets), relying on the
editor / code-viewer's URL autolinkification rather than markdown
rendering. See `plain_comment_form` below.

## What to lint

Scan every `///` and `//!` doc comment — and, when
`include_plain_comments = true`, every plain `//` comment too —
for a `#` followed by one or more ASCII digits, ending at a word
boundary, and not already preceded by a word character or by the
`[` character (the opening of an existing markdown link). For
each match outside a code span / code block, emit a diagnostic
at the bare-reference span.

Skip:

- Inside `` `...` `` code spans and ` ``` ... ``` ` fences.
- Inside an existing intra-doc link or markdown link
  (`[#123]`, `[#123](...)`, `[label](#123)`).
- Inside a markdown reference-link definition trailing target
  (`[#123]: https://...`).
- When the match is the fragment of a written-out URL — applies
  whether the URL appears as a bare URL in doc text or wrapped
  as a markdown link. Note that the left-context guard already
  catches the common case where the `#` is preceded by a word
  character (e.g., `https://example.com/article#123` — the `e`
  before `#` is a word char, so the bare match never fires); the
  URL-fragment skip exists for fragments whose `#` is preceded
  by a non-word character that the left-context guard alone
  would let through, e.g.
  `https://example.com/issues/#123`,
  `https://example.com/path?ref=#123`.

When `include_plain_comments = true`, the same token-shape match
runs over `//` comment text as well. Markdown-specific skips
(code spans, existing markdown links, reference-link definitions)
don't apply there — plain comments aren't markdown — but the
left-context guard (no word character, no `[` before the `#`)
and the URL-fragment skip still do.

## Examples

```rust
// Bad
/// Closes #123 and supersedes #124.

// Good (inline form)
/// Closes [#123](https://github.com/owner/repo/issues/123) and
/// supersedes [#124](https://github.com/owner/repo/issues/124).

// Good (reference form, definitions at the end of the doc block)
/// Closes [#123] and supersedes [#124].
///
/// [#123]: https://github.com/owner/repo/issues/123
/// [#124]: https://github.com/owner/repo/issues/124
```

With `include_plain_comments = true`:

```rust
// Bad
// Workaround for #123; revisit once upstream lands #124.

// Good (plain_comment_form = "bare")
// Workaround for https://github.com/owner/repo/issues/123; revisit
// once upstream lands https://github.com/owner/repo/issues/124.

// Good (plain_comment_form = "bracketed")
// Workaround for <https://github.com/owner/repo/issues/123>; revisit
// once upstream lands <https://github.com/owner/repo/issues/124>.
```

## Configuration

```toml
[bare_issue_reference]
# Base URL used to construct the suggested target. Required for
# `MachineApplicable` autofix. When unset, every `suggestion_mode`
# degrades to help-only output (no URL can be constructed),
# so the lint stays usable with zero configuration — it just
# flags bare references for manual triage.
repo_base_url = "https://github.com/owner/repo"

# Templates for the suggested URL. `{number}` is substituted.
issue_url_template = "{repo_base_url}/issues/{number}"
pr_url_template    = "{repo_base_url}/pull/{number}"

# How the lint suggests the link.
suggestion_mode = "issue_url"
# "issue_url"  — single MachineApplicable suggestion using
#                `issue_url_template`. Safe on GitHub because
#                `/issues/<n>` redirects to `/pull/<n>` when the
#                number names a PR.
# "both"       — emit two MaybeIncorrect suggestions (one issue
#                URL, one PR URL) and let the author pick. Use on
#                forges that don't redirect (some GitLab self-hosts).
# "help_only"  — emit no Suggestion, only help text. Use when
#                `repo_base_url` cannot be configured statically
#                (e.g., a workspace with multiple repositories).
#                Distinct from the unset-`repo_base_url`
#                degradation described above: setting this mode
#                explicitly keeps the lint help-only even when
#                `repo_base_url` is configured.

# Defaults to "inline". When set to "reference", the lint emits the
# `[#N] / [#N]: <url>` two-piece form instead of `[#N](<url>)`.
form = "inline"

# When true, also lint plain `//` line comments. The autofix in
# plain comments cannot use markdown `[#N](url)` syntax (plain
# comments aren't markdown-rendered), so the substitution uses
# the URL form selected by `plain_comment_form` instead. `form`
# only governs doc-comment fixes and is ignored for plain
# comments. Under `suggestion_mode = "help_only"` (or whenever
# `repo_base_url` is unset), plain-comment diagnostics are still
# emitted help-only; no URL substitution is attempted.
# Plain *block* comments (`/* ... */`) are out of scope for this
# lint regardless of this setting. The sibling `bare_url`'s plan
# is to scan block comments by reusing the regular-comment
# retokenizer of the already-implemented
# `perfectionist::unicode_ellipsis_in_comments`; this rule
# deliberately doesn't, because the working assumption is that
# `#NNN` references in Rust code live in `//` and `///` comments.
# If a real codebase surfaces block-comment references, revisit.
include_plain_comments = false

# Replacement form used inside plain `//` comments when
# `include_plain_comments = true`. Ignored for doc comments and
# ignored whenever no URL can be substituted (see the unset-
# `repo_base_url` / `help_only` notes above).
plain_comment_form = "bare"
# "bare"      — substitute the bare URL (https://...). Many editors
#               and code-view UIs auto-detect bare URLs as
#               clickable, but they disagree on whether trailing
#               punctuation (`;`, `,`, `.`) belongs to the URL;
#               pick "bracketed" if the surrounding prose tends to
#               put punctuation immediately after the reference.
# "bracketed" — substitute <https://...>. A pre-markdown convention
#               for delimiting a URL inside prose; the angle
#               brackets give the URL a clear boundary when it
#               abuts surrounding punctuation.
```

## Implementation notes

- `LateLintPass::check_attribute` to read `#[doc = "..."]`
  attribute values. Walk the rendered text once per item, using
  the shared markdown scanner (Tier A — structural classification)
  per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md#markdown-parsing)
  to skip code regions, existing links, and reference-link
  definitions.
- When `include_plain_comments = true`, additionally walk plain
  `//` comments via an `EarlyLintPass` over the source map (or a
  pre-expansion source-text walk over each file's comment tokens).
  The markdown scanner is not invoked here; instead reuse the
  same `take_*` token scanner over the raw comment text.
- **URL-fragment detection.** Run this check in the same pass as
  the markdown scanner's structural classification (the bare URL
  it protects is plain markdown text the structural scanner
  doesn't classify as a skip region — so a post-filter ordering
  would miss it). For each `#N` candidate in either target, walk
  backward from the `#` — bounded to the current comment line's
  text content — for a `<ASCII letters>://` prefix with no
  intervening whitespace. If found, skip the match. The check
  covers `http`, `https`, `ftp`, `git`, `ssh`, and any other
  scheme that lands in source comments. A `#NNN` on the line
  after a URL that wrapped across `//` lines is flagged (the
  rule treats line breaks as URL terminators, deliberately giving
  up on wrapped-URL fragments).
  [`perfectionist::bare_url`](./bare-url.md)'s planned scanner
  commits forward from the scheme; the two scanners walk in
  opposite directions but agree on what counts as a URL run. If
  either rule grows past the trivial "scheme + non-whitespace"
  check, factor URL discovery into a crate-internal helper
  shared by the two rules rather than duplicating the scanner.
- The autofix substitutes the bare span with the rendered link.
  Suggestion applicability:
  - `suggestion_mode = "issue_url"` → `MachineApplicable`. The
    `/issues/<n>` URL works for both issues and PRs on GitHub thanks
    to redirect; on configured non-GitHub forges, mark this mode as
    `MaybeIncorrect` and emit a note explaining why.
  - `suggestion_mode = "both"` → two `MaybeIncorrect` suggestions.
    `cargo clippy --fix` will not apply either; the author picks
    manually.
  - `suggestion_mode = "help_only"` → no Suggestion. Just point at
    the span and explain the requirement.
- Reading `repo_base_url` from `Cargo.toml`'s `[package].repository`
  is *not* attempted by the lint pass (a Dylint pass cannot read
  arbitrary files). Document the pattern of duplicating the URL in
  `dylint.toml`, and offer a small build-script snippet in the
  project's README that synchronises the two.
- The bare-reference scanner itself is trivial enough (a literal
  `#`, a digit run, and a one-byte left-context check) that the
  parser-combinator scaffolding called out in
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  is not required. The URL-fragment backward walk and the
  markdown structural classifier are the parts of this rule that
  do follow the combinator convention.
- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Interaction with sibling rules

When `include_plain_comments = true` and `plain_comment_form = "bare"`,
the autofix produces a bare URL inside a plain comment — exactly
what [`perfectionist::bare_url`](./bare-url.md) is designed to
flag. Since `bare_url`'s default `targets = ["doc", "comment"]`
includes the `"comment"` target, the rewrite chain
`#123 → https://... → <https://...>` materialises iteratively
across `cargo dylint --fix` invocations (the second hop only
fires on a re-run). A CI that runs `--fix` once and immediately
re-lints will see a `bare_url` violation on the intermediate
output and fail.

Set `plain_comment_form = "bracketed"` to make the first fix
already produce `<https://...>`, which both rules accept. A
project that has consciously decided bare URLs in comments are
fine can instead narrow `bare_url`'s `targets` or add the issue
host to its `skip_hosts` — those choices are valid project
postures, just broader in effect than the `"bracketed"` swap.

## Default state

Active by default. With `repo_base_url` unset, every
`suggestion_mode` degrades to help-only output, so the lint is
adoptable with zero configuration (see `repo_base_url` in
Configuration).
