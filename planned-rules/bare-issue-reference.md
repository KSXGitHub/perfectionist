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

In both doc and plain comments, the match is case-insensitive
when alternative tokens like `GH-`, `gh-`, or `pr#` are
configured.

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
# degrades to help-text-only output (no URL can be constructed),
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
#                Selecting this mode explicitly is independent of
#                the unset-`repo_base_url` degradation described
#                above — the explicit form is portable across
#                configurations where `repo_base_url` happens to
#                be set.

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
# comments. Under `suggestion_mode = "help_only"` (or whenever
# `repo_base_url` is unset), plain-comment diagnostics are still
# emitted help-text-only; no URL substitution is attempted.
# Plain *block* comments (`/* ... */`) are out of scope for this
# lint regardless of this setting — issue references appear in
# `//` and `///` comments in practice, not `/* ... */` ones, and
# narrowing the scope avoids speculative scope growth without a
# real adopter need.
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
- **URL-fragment detection.** For each `#N` candidate (in either
  target, and at the same step as the markdown scanner's skip
  decisions for the doc-comment target — not after them — so
  bare URLs in doc text are recognised too), walk backward from
  the `#` looking for `https://` or `http://` with no intervening
  whitespace. If the candidate sits inside that contiguous run,
  skip the match. [`perfectionist::bare_url`](./bare-url.md)
  already needs a forward URL scanner; if either rule grows past
  the trivial "backward to scheme, no whitespace" check, factor
  URL discovery into a crate-internal helper shared by the two
  rules rather than duplicating the scanner.
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
flag. Since `bare_url`'s default `targets = ["doc", "comment"]`
includes the `"comment"` target, the rewrite chain
`#123 → https://... → <https://...>` materialises iteratively
across `cargo dylint --fix` invocations (the second hop only
fires on a re-run). A CI that runs `--fix` once and immediately
re-lints will see a `bare_url` violation on the intermediate
output and fail.

Set `plain_comment_form = "bracketed"` to make the first fix
already produce `<https://...>`, which both rules accept. Per-
project relaxations are available too — narrowing `bare_url`'s
`targets` or adding the URL to its `skip_hosts` — but those
disable `bare_url` checking beyond just this rule's autofix
output and are usually the wrong knob.

The two rules' planning files also disagree on plain `/* ... */`
block-comment scope: `bare_url` scans block comments by default;
this rule does not (see the `include_plain_comments` knob for
the rationale).

## Default state

Active by default.
