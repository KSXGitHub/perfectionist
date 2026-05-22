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
# Defaults: 6 to 40, accepting any abbreviation that's at least
# Git's default-render length (6 chars in `git log --oneline`,
# bumped from Git's hard minimum of 4 to push back on
# overly-short SHAs that risk ambiguity in large repos).
#
# To require the full 40-char SHA:
#   commit_length_min = 40, commit_length_max = 40
#
# To require a specific abbreviated length (keeps URLs short while
# staying pinned):
#   commit_length_min = 12, commit_length_max = 12
#
# To allow any length within a tighter window:
#   commit_length_min = 7, commit_length_max = 12
commit_length_min = 6
commit_length_max = 40

# Forge hosts to scan, mapped to the forge kind that determines the
# URL shapes containing SHA-shaped path segments. Each kind's
# patterns (file, tree, raw, commit, compare, …) are hardcoded.
# Configuration declares hostnames only; self-hosted instances
# register by adding an entry with the appropriate kind.
hosts = [
  { hostname = "github.com",    kind = "github" },
  { hostname = "gitee.com",     kind = "github" },     # github-shape
  { hostname = "gitlab.com",    kind = "gitlab" },
  { hostname = "bitbucket.org", kind = "bitbucket" },
  { hostname = "codeberg.org",  kind = "gitea" },      # gitea-shape
  { hostname = "git.sr.ht",     kind = "sourcehut" },
]

# Skip URLs whose host matches one of these glob patterns.
skip_hosts = []

# Skip refs that are not pure hex even if they appear in a slot
# the per-kind URL shape marks as a SHA. By default the lint
# treats a non-hex ref as "not a SHA, this rule has nothing to
# say"; another rule (typically `unpinned-repo-ref`) handles the
# branch case. Set to false to flag non-hex refs in SHA slots as
# wrong-shape.
ignore_non_hex_refs = true
```

A self-hosted instance registers with one entry:

```toml
hosts = [
  { hostname = "gitlab.example.com", kind = "gitlab" },
  { hostname = "git.example.com",    kind = "gitea" },
]
```

The `hostname` field accepts a glob (`gitlab.*.example.com`).

### SHA-bearing URL paths per forge kind

The hardcoded SHA slots (and compare-range slots) by kind:

| Kind        | URL paths containing SHAs                                                                                                  |
|-------------|----------------------------------------------------------------------------------------------------------------------------|
| `github`    | `/{owner}/{repo}/(blob|tree|raw|edit|blame)/{sha}/...`, `/commit/{sha}`, `/commits/{sha}`, `/compare/{sha_a}(..|...){sha_b}` |
| `gitlab`    | `/{owner}/{repo}/-/(blob|tree|raw|edit)/{sha}/...`, `/-/commit/{sha}`, `/-/compare/{sha_a}...{sha_b}`                       |
| `bitbucket` | `/{owner}/{repo}/src/{sha}/...`, `/commits/{sha}`, `/branches/compare/{sha_a}..{sha_b}`                                     |
| `gitea`     | `/{owner}/{repo}/(src|raw)/commit/{sha}/...`, `/commit/{sha}`, `/compare/{sha_a}...{sha_b}`                                 |
| `sourcehut` | `/~{user}/{repo}/tree/{sha}/item/...`, `/commit/{sha}`                                                                      |

## What to lint

For every URL whose host matches a configured `hosts` entry,
dispatch to the per-kind matcher. For each SHA slot (and each side
of a compare range) it captures:

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
// Default config (any length 6..=40): all of these pass.
/// See <https://github.com/owner/repo/commit/8c1f6e2>.
/// See <https://github.com/owner/repo/commit/8c1f6e2a6d33c1b1a2f9e0e1d3b8a4c7d6e5f4a3>.
/// See <https://github.com/owner/repo/compare/abcdef0...feedface>.

// Bad under default 6..=40: SHA shorter than the minimum.
/// See <https://github.com/owner/repo/commit/abc>.

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
  `perfectionist::bare_url` (`src/url_scan.rs`) and
  [`unpinned-repo-ref`](./unpinned-repo-ref.md). Discovery happens
  once per source comment; classification is per-lint.
- Each `kind` has a small, fixed set of SHA-bearing path patterns,
  baked into the lint at compile time. The lint does not parse user-
  supplied templates; configuration declares hostnames only.
- For compare URLs, emit one diagnostic per offending SHA. The same
  URL may produce two warnings.
- The wrong-length diagnostic span is the SHA itself, not the whole
  URL, so editors can highlight just the bad portion.
- **Parser style.** Implement the URL matcher as parser-
  combinator-style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
  reuse the URL skeleton from
  [`unpinned-repo-ref`](./unpinned-repo-ref.md), then add
  `take_sha` (a run of `[0-9a-fA-F]`) and `take_range_separator`
  (`...` or `..`) for the compare-URL case. The per-kind matchers
  are small functions registered in a table keyed by kind.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

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

## Default state

Active by default. The default window (`6..=40`) rejects SHAs
shorter than Git's default abbreviation length while accepting
everything from 6-char prefixes up to the full 40-char hash; a
project tightens the window further by raising the minimum or
pinning a fixed length.

## Interaction with `unpinned-repo-ref`

The two lints are orthogonal and run independently. A URL may emit:

- A `unpinned-repo-ref` diagnostic if its ref is a branch.
- A `commit-id-length` diagnostic if its ref is a SHA outside the
  window.
- Both, in the unusual case where the URL contains both a branch ref
  and a SHA in different slots (e.g., a compare URL with one branch
  and one SHA — which most forges support).
