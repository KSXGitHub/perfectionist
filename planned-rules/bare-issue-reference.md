# `bare_issue_reference`

**Source:** project convention.

## Statement

In doc comments (`///`, `//!`), references to issues or pull requests
written as the bare token `#<digits>` (or a configured equivalent like
`GH-123`) must be rendered as markdown links. Two acceptable forms:

- Inline: `[#123](https://github.com/owner/repo/issues/123)`.
- Reference: `[#123]` paired with `[#123]: https://github.com/owner/repo/issues/123`.

The rule applies only to *doc comments*. Plain `//` comments are
out of scope (they don't render anywhere) and are not linked anyway.

## What to lint

Scan every `///` and `//!` comment for the regex equivalent of
`(?<![\w\[])#\d+\b` (a `#` followed by digits, not already preceded
by a word character or an opening bracket). For each match outside
a code span / code block, emit a diagnostic at the bare-reference
span.

Skip:

- Inside `` `...` `` code spans and ` ``` ... ``` ` fences.
- Inside an existing intra-doc link or markdown link
  (`[#123]`, `[#123](...)`, `[label](#123)`).
- Inside a markdown reference-link definition trailing target
  (`[#123]: https://...`).

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

# Additional bare-reference tokens to recognise. Default empty.
extra_tokens = []                # e.g., ["GH-", "gh-", "pr#"]

# Defaults to "inline". When set to "reference", the lint emits the
# `[#N] / [#N]: <url>` two-piece form instead of `[#N](<url>)`.
form = "inline"
```

## Implementation notes

- `LateLintPass::check_attribute` to read `#[doc = "..."]` attribute
  values. Walk the rendered text once per item, applying the markdown
  exclusion logic from
  [`intra-doc-links`](./intra-doc-links.md) and
  [`unicode-ellipsis-in-docs`](./unicode-ellipsis-in-docs.md). Share
  the helper.
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
  preceding byte is not a word character or an opening bracket.
  Each `extra_tokens` entry becomes one alternative in the
  `take_token_prefix` step.

## Severity

Warn. With `repo_base_url` unset and `suggestion_mode = "help_only"`,
the lint becomes informational and a project can run it without
configuration to flag the bare references for manual triage.
