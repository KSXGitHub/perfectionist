# `unpinned_repo_ref`

**Default state:** `active`  
**Source:** project convention. The pacquet `CODE_STYLE_GUIDE.md` calls
the principle out as a *cardinal rule* — every upstream permalink must
be pinned to a SHA — but the rule itself appears across many
codebases.

## Statement

A URL that references a file or directory inside a hosted git
repository must use a **commit SHA** as its ref. Anything else is
rejected:

- Branch names (`main`, `master`, `develop`, `my-feature`) are
  rejected because the file at that path will change without warning
  when the branch advances.
- Tag names (`v1.2.3`, `release-2024-01`) are also rejected. Tags
  cannot be reliably distinguished from branches by name alone — a
  branch named `v1.2.3` is perfectly valid Git, and the lint cannot
  query the remote to disambiguate. Treating tags as pinned would
  open a false-negative window the rule cannot afford.

The rule applies to the common forges:

- GitHub: `https://github.com/{owner}/{repo}/(blob|tree|raw|edit|blame)/{ref}/{path}`
- GitLab (gitlab.com or self-hosted): `https://gitlab.com/{owner}/{repo}/-/(blob|tree|raw|edit)/{ref}/{path}`
- Bitbucket: `https://bitbucket.org/{owner}/{repo}/src/{ref}/{path}`
- Gitea / Codeberg / Forgejo: `https://codeberg.org/{owner}/{repo}/src/(branch|commit|tag)/{ref}/{path}` — the URL itself names the ref kind, which the lint uses directly.
- sourcehut: `https://git.sr.ht/~{user}/{repo}/tree/{ref}/item/{path}`
- gitee: same shape as GitHub.

This rule is concerned **only** with whether the ref is mutable. The
*length* of an accepted SHA is a separate concern handled by
[`commit-id-length`](./commit-id-length.md), which also covers
commit-only and compare-only URLs (`/commit/<sha>`,
`/compare/<sha>...<sha>`) that this lint deliberately excludes.

## What to lint

For every URL whose host is a configured forge host, locate the ref
segment (per the forge's hardcoded URL shape) and classify it:

- **Commit SHA**: pure hex, length at least `sha_recognition_length`
  (default 4 — Git's own minimum SHA length). Accept.
- **Gitea-style ref-typed URL**: when the URL contains
  `/src/branch/`, the ref is *known* to be a branch and is rejected.
  `/src/commit/` proceeds to the SHA-shape check (so a non-hex value
  in that slot is still rejected). `/src/tag/` is rejected
  outright — tags are not accepted by this rule.
- **Anything else** (branch names, tag names): reject.

Diagnostic span is the ref substring of the URL, with help text
that explains a SHA is expected.

## Examples

```rust
// Bad: branch ref
/// See <https://github.com/owner/repo/blob/main/src/lib.rs>.

// Bad: tag ref (cannot be distinguished from a branch by name)
/// See <https://github.com/owner/repo/blob/v1.2.3/src/lib.rs>.

// Good: long SHA
/// See <https://github.com/owner/repo/blob/8c1f6e2a6d33c1b1a2f9e0e1d3b8a4c7d6e5f4a3/src/lib.rs>.

// Good: short SHA (any length the lint recognises as hex)
/// See <https://github.com/owner/repo/blob/8c1f6e2/src/lib.rs>.

// Bad on Gitea-style: explicit `/src/branch/`
/// See <https://codeberg.org/owner/repo/src/branch/main/src/lib.rs>.

// Bad on Gitea-style: explicit `/src/tag/`
/// See <https://codeberg.org/owner/repo/src/tag/v1.2.3/src/lib.rs>.

// Good on Gitea-style: explicit `/src/commit/`
/// See <https://codeberg.org/owner/repo/src/commit/8c1f6e2.../src/lib.rs>.
```

## Configuration

```toml
[unpinned_repo_ref]
# Where the lint scans. Subset of these.
targets = ["doc", "comment", "string_literal"]

# Minimum hex length for a ref to be recognised as a SHA. Below this
# the ref is treated as "not obviously a SHA" and rejected as a
# branch. The default of 4 matches Git's own minimum SHA length and
# trades a small false-negative window (branch names like `dead`,
# `face`, `beef`) for fewer false positives on similar branch
# names. Set to 1 to treat any pure-hex string as a SHA (aggressive).
sha_recognition_length = 4

# Hosts to scan, mapped to the forge kind that determines the URL
# shape. Each kind's URL patterns (file, tree, raw, edit, blame, …)
# are hardcoded; configuration only declares which hostname maps to
# which kind. Self-hosted instances register by adding an entry with
# the same kind.
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
```

A self-hosted GitLab is registered with one entry:

```toml
hosts = [
  { hostname = "gitlab.example.com", kind = "gitlab" },
]
```

A self-hosted Gitea / Forgejo instance:

```toml
hosts = [
  { hostname = "git.example.com", kind = "gitea" },
]
```

The `hostname` field accepts a glob (`gitlab.*.example.com`) so
subdomains can be covered in one entry.

### Forge kinds

The hardcoded URL shapes by kind:

| Kind        | File-reference paths                                                          |
|-------------|-------------------------------------------------------------------------------|
| `github`    | `/{owner}/{repo}/(blob|tree|raw|edit|blame)/{ref}/{path}`                     |
| `gitlab`    | `/{owner}/{repo}/-/(blob|tree|raw|edit)/{ref}/{path}`                         |
| `bitbucket` | `/{owner}/{repo}/src/{ref}/{path}`                                            |
| `gitea`     | `/{owner}/{repo}/(src|raw)/(branch|commit|tag)/{ref}/{path}` — ref-typed     |
| `sourcehut` | `/~{user}/{repo}/(tree|blob)/{ref}/item/{path}`                                |

The `gitea` kind's URL paths encode the ref kind directly: a
`/src/branch/X` URL has X as a known branch, `/src/commit/X` has X
as a known commit, `/src/tag/X` has X as a known tag. This lint
rejects branch and tag URLs unconditionally and accepts commit URLs
when the ref is hex.

## Implementation notes

- `LateLintPass`. Reuse the URL discovery logic from
  [`bare-url`](./bare-url.md) — same scanner, same code-span / code-
  block exclusions, same retokenization for regular comments. The
  same URL match feeds this lint, `bare-url`, and
  `commit-id-length`.
- Parse each candidate URL with `url::Url` or a small hand-written
  parser. Match the host against the configured `hosts` list (after
  glob expansion); look up the matched entry's `kind`; dispatch to
  the per-kind URL matcher.
- Each `kind` has a small, fixed set of recognised path patterns,
  baked into the lint at compile time. The lint does not parse user-
  supplied templates; configuration declares hostnames only.
- Ref classification (in order):
  1. Gitea-style ref-typed URL: classification is forced by the
     URL path. `/src/branch/` and `/src/tag/` are always rejected;
     `/src/commit/` proceeds to the SHA shape check.
  2. SHA: pure hex, length at least `sha_recognition_length`.
  3. Otherwise: branch (rejected).
- Diagnostic span pinpoints the ref substring, with a help message
  recommending the pinned form. **No autofix.** A Dylint pass cannot
  resolve a branch to a SHA without shelling out to `git` or the
  forge's API; both would be out of scope and would also be pointing
  at the wrong thing (the URL might reference a *different*
  repository than the local one).
- **Parser style.** Implement the URL matcher as parser-combinator-
  style `take_*` functions per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
  `take_scheme`, `take_host`, then a per-kind sequence of
  `take_segment` (matching a literal like `blob` or `tree`) and
  `take_capture` (extracting `{owner}`, `{repo}`, `{ref}`). The
  per-kind matchers are five small functions, one per supported
  forge kind — they're registered in a table keyed by kind and
  invoked after the host lookup.

- See [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for cross-cutting conventions that apply to every rule in this
  catalogue, in particular the lint-name namespacing (`perfectionist::*`)
  that every registered lint follows.

## Interaction with sibling lints

- [`bare-url`](./bare-url.md) ensures the URL is wrapped (`<...>` or
  labelled).
- [`commit-id-length`](./commit-id-length.md) ensures any commit SHA
  in the URL has the project's required length.
- `unpinned_repo_ref` (this rule) ensures the wrapped URL points at
  a pinned ref rather than a branch or tag.

A URL that fails several of these rules produces several
diagnostics, one per concern. The lints layer rather than overlap.
