# `bare_issue_reference`

**Source:** project convention.

## Statement

In doc comments (`///`, `//!`), references to issues or pull requests
written as the bare token `#<digits>` (or a configured equivalent like
`GH-123`) must be rendered as markdown links. Two acceptable forms:

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

Scan every `///` and `//!` comment for a `#` followed by one or
more ASCII digits, ending at a word boundary, and not already
preceded by a word character or by the `[` character (the
opening of an existing markdown link). For each match outside a
code span / code block, emit a diagnostic at the bare-reference
span.

Skip:

- Inside `` `...` `` code spans and ` ``` ... ``` ` fences.
- Inside an existing intra-doc link or markdown link
  (`[#123]`, `[#123](...)`, `[label](#123)`).
- Inside a markdown reference-link definition trailing target
  (`[#123]: https://...`).
- When the match is the fragment of a written-out URL (e.g., the
  `#123` in `https://example.com/article#123`) — applies whether
  the URL appears as a bare URL in doc text or wrapped as a
  markdown link.

When `include_plain_comments = true`, the same token-shape match
runs over `//` comment text as well. Markdown-specific skips
(code spans, existing markdown links, reference-link definitions)
don't apply there — plain comments aren't markdown — but the
left-context guard (no word character, no `[` before the `#`)
and the URL-fragment skip still do.

The match is case-insensitive when alternative tokens like `GH-`,
`gh-`, or `pr#` are configured.

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
# `MachineApplicable` autofix.
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
#                With `repo_base_url` unset, this mode runs the
#                lint informationally to flag bare references for
#                manual triage with no further configuration.

# Additional bare-reference tokens to recognise. Default empty.
extra_tokens = []                # e.g., ["GH-", "gh-", "pr#"]

# Defaults to "inline". When set to "reference", the lint emits the
# `[#N] / [#N]: <url>` two-piece form instead of `[#N](<url>)`.
form = "inline"

# When true, also lint plain `//` line comments. The autofix in
# plain comments cannot use markdown `[#N](url)` syntax (plain
# comments aren't markdown-rendered), so the substitution uses
# the URL form selected by `plain_comment_form` instead. `form`
# only governs doc-comment fixes and is ignored for plain
# comments. Plain *block* comments (`/* ... */`) are out of
# scope for this lint regardless of this setting.
include_plain_comments = false

# Replacement form used inside plain `//` comments when
# `include_plain_comments = true`. Ignored for doc comments.
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
  same `take_*` token scanner over the raw comment text. The
  URL-fragment skip (described in "What to lint" above) applies
  to both targets.
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
- **Parser style.** Decompose the bare-reference scanner into
  parser-combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
  `take_token_prefix` (`#`, `GH-`, `gh-`, or any user-configured
  alternative), `take_digits`, and a left-context check that the
  preceding byte is not a word character or a `[`. Each
  `extra_tokens` entry becomes one alternative in the
  `take_token_prefix` step.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Interaction with sibling rules

When `include_plain_comments = true` and `plain_comment_form = "bare"`,
the autofix produces a bare URL inside a plain comment — exactly
what [`perfectionist::bare_url`](./bare-url.md) is designed to
flag. With both rules enabled and `bare_url`'s `targets` including
`"comment"`, the rewrite chain is `#123 → https://... → <https://...>`
across two passes. To avoid the second pass, set
`plain_comment_form = "bracketed"`, which produces the
`<https://...>` form directly.

## Default state

Active by default.
