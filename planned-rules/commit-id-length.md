# `commit_id_length`

**Source:** project convention. Sibling lint to
[`unpinned-repo-ref`](./unpinned-repo-ref.md), which decides whether
a forge URL's ref is pinned at all. This rule decides whether the
SHA *is the right length*.

## Statement

When a URL references a commit by SHA — whether to pin a file/directory
view, to show a single commit, or to compare a range — the SHA's
length must fall in the project's configured window. The window can
be set to "any length" (most permissive), to a fixed value (most
strict, e.g., always 12 chars to keep URLs short while still pinned),
or to a range.

The rule applies to every URL that contains a SHA-shaped path
segment, including:

- File and directory references: `/blob/<sha>/...`, `/tree/<sha>/...`,
  `/raw/<sha>/...`, `/edit/<sha>/...`, `/blame/<sha>/...`, and the
  GitLab-style equivalents under `/-/`.
- Single-commit views: `/commit/<sha>`, `/commits/<sha>`,
  `/-/commit/<sha>`.
- Range comparisons: `/compare/<sha>...<sha>`,
  `/compare/<sha>..<sha>` (two-dot range), `/-/compare/<sha>...<sha>`.
- Codeberg-style `/src/commit/<sha>/...` and `/raw/commit/<sha>/...`.

This is broader than `unpinned-repo-ref`'s scope: a `/commit/<sha>`
URL has no "is it pinned?" question to answer (a commit URL is
trivially pinned), but it *does* have a "is the SHA the right
length?" question that should be answered consistently across the
codebase.

## Configuration

```toml
[commit_id_length]
# Where the lint scans. Subset of these.
targets = ["doc", "comment", "string_literal"]

# Range for the SHA length, inclusive.
#
# To accept any length (default behaviour — no enforcement):
#   commit_length_min = 1, commit_length_max = 40
#
# To require the full 40-char SHA:
#   commit_length_min = 40, commit_length_max = 40
#
# To require a specific abbreviated length (keeps URLs short while
# staying pinned):
#   commit_length_min = 12, commit_length_max = 12
#
# To allow any length within a window:
#   commit_length_min = 7, commit_length_max = 12
commit_length_min = 1
commit_length_max = 40

# Forge URL patterns. Each entry maps a host glob to one or more
# path templates that locate SHA-shaped segments. `{sha}` matches a
# single hex segment; `{sha_a}` and `{sha_b}` match the two SHAs in
# a compare URL. `**` matches any path suffix.
forges = [
  { host = "github.com", paths = [
      "{owner}/{repo}/blob/{sha}/**",
      "{owner}/{repo}/tree/{sha}/**",
      "{owner}/{repo}/raw/{sha}/**",
      "{owner}/{repo}/edit/{sha}/**",
      "{owner}/{repo}/blame/{sha}/**",
      "{owner}/{repo}/commit/{sha}",
      "{owner}/{repo}/commits/{sha}",
      "{owner}/{repo}/compare/{sha_a}...{sha_b}",
      "{owner}/{repo}/compare/{sha_a}..{sha_b}",
  ] },
  { host = "gitlab.com", paths = [
      "{owner}/{repo}/-/blob/{sha}/**",
      "{owner}/{repo}/-/tree/{sha}/**",
      "{owner}/{repo}/-/raw/{sha}/**",
      "{owner}/{repo}/-/edit/{sha}/**",
      "{owner}/{repo}/-/commit/{sha}",
      "{owner}/{repo}/-/compare/{sha_a}...{sha_b}",
  ] },
  { host = "bitbucket.org", paths = [
      "{owner}/{repo}/src/{sha}/**",
      "{owner}/{repo}/commits/{sha}",
      "{owner}/{repo}/branches/compare/{sha_a}..{sha_b}",
  ] },
  { host = "codeberg.org", paths = [
      "{owner}/{repo}/src/commit/{sha}/**",
      "{owner}/{repo}/raw/commit/{sha}/**",
      "{owner}/{repo}/commit/{sha}",
      "{owner}/{repo}/compare/{sha_a}...{sha_b}",
  ] },
  { host = "gitee.com", paths = [
      "{owner}/{repo}/blob/{sha}/**",
      "{owner}/{repo}/commit/{sha}",
      "{owner}/{repo}/compare/{sha_a}...{sha_b}",
  ] },
  { host = "git.sr.ht", paths = [
      "~{user}/{repo}/tree/{sha}/item/**",
      "~{user}/{repo}/commit/{sha}",
  ] },
]

# Skip URLs whose host matches one of these glob patterns.
skip_hosts = []

# Skip refs that are not pure hex even if they appear in a slot the
# template marks as `{sha}`. By default the lint treats a non-hex
# ref as "not a SHA, this rule has nothing to say"; another rule
# (typically `unpinned-repo-ref`) handles the branch case. Set to
# false to flag non-hex refs in `{sha}` slots as wrong-shape.
ignore_non_hex_refs = true
```

## What to lint

For every URL that matches a configured forge template, walk the
captured `{sha}` (and `{sha_a}`/`{sha_b}` for compare URLs) groups.
For each capture:

1. If the captured value is not pure hex and `ignore_non_hex_refs`
   is true, skip — `unpinned-repo-ref` will handle the branch case
   if applicable.
2. If the captured value is pure hex but its length falls outside
   `[commit_length_min, commit_length_max]`, emit a diagnostic at
   the SHA's substring, naming the configured window and the actual
   length.

A compare URL emits up to two diagnostics, one per SHA.

## Examples

```rust
// Default config (any length 1..=40): all of these pass.
/// See <https://github.com/owner/repo/commit/8c1f6e2>.
/// See <https://github.com/owner/repo/commit/8c1f6e2a6d33c1b1a2f9e0e1d3b8a4c7d6e5f4a3>.
/// See <https://github.com/owner/repo/compare/abcdef0...feedface>.

// Under `commit_length_min = 12, commit_length_max = 12`:
//   the 7-char SHA is flagged; the 40-char SHA is also flagged
//   because it's longer than 12.
/// See <https://github.com/owner/repo/commit/8c1f6e2>.            // bad
/// See <https://github.com/owner/repo/commit/8c1f6e2a6d33>.       // good
/// See <https://github.com/owner/repo/commit/8c1f6e2a6d33c1...>.  // bad

// Under `commit_length_min = 40, commit_length_max = 40`:
//   only full SHAs accepted.
/// See <https://github.com/owner/repo/blob/8c1f6e2/file.rs>.      // bad
```

## Implementation notes

- `LateLintPass`. Share the URL scanner with
  [`bare-url`](./bare-url.md) and
  [`unpinned-repo-ref`](./unpinned-repo-ref.md). Discovery happens
  once per source comment; classification is per-lint.
- The forge template parser must support both `{sha}` (single
  capture) and `{sha_a}` / `{sha_b}` (two captures separated by
  `..` or `...`). Implement as a small ad-hoc matcher rather than a
  full glob library.
- For compare URLs, emit one diagnostic per offending SHA. The same
  URL may produce two warnings.
- The wrong-length diagnostic span is the SHA itself, not the whole
  URL, so editors can highlight just the bad portion.
- **Parser style.** Implement the forge template matcher as
  parser-combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
  reuse the URL skeleton from
  [`unpinned-repo-ref`](./unpinned-repo-ref.md), then add
  `take_sha` (a run of `[0-9a-fA-F]`) and `take_range_separator`
  (`...` or `..`) for the compare-URL case. A `{sha_a}...{sha_b}`
  template becomes `take_sha`, `take_range_separator`, `take_sha`
  in sequence — the combinator order makes the two-SHA capture
  obvious to the reader.

## Autofix

`MachineApplicable` is offered in one specific case: the existing
SHA is **longer** than `commit_length_max`, and `commit_length_min
== commit_length_max` (a fixed length is configured). In that case
the lint can truncate the SHA to the configured length without
losing pinning — `8c1f6e2a6d33c1...` truncated to 12 chars is still
unambiguous on the same repository.

The reverse case — the SHA is **shorter** than the required length —
cannot be auto-fixed because the lint pass cannot synthesise the
missing characters without consulting git. Emit help text only.

When the configured window is a range rather than a fixed length,
auto-truncation is also off; the author must pick a length within
the window themselves.

## Severity

Warn. The defaults (`1..=40`) make the lint a no-op; a project opts
in by tightening the window.

## Interaction with `unpinned-repo-ref`

The two lints are orthogonal and run independently. A URL may emit:

- A `unpinned-repo-ref` diagnostic if its ref is a branch.
- A `commit-id-length` diagnostic if its ref is a SHA outside the
  window.
- Both, in the unusual case where the URL contains both a branch ref
  and a SHA in different slots (e.g., a compare URL with one branch
  and one SHA — which most forges support).
