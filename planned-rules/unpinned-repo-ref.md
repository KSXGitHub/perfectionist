# `unpinned_repo_ref`

**Source:** project convention. The pacquet `CODE_STYLE_GUIDE.md` calls
the principle out as a *cardinal rule* — every upstream permalink must
be pinned to a SHA — but the rule itself appears across many
codebases.

## Statement

A URL that references a file or directory inside a hosted git
repository must use a pinned ref:

- A long commit SHA (40 hex chars).
- A short commit SHA (7 to 39 hex chars; minimum length is
  configurable).
- Optionally a tag, when the project trusts that tags are not force-
  moved.

A URL that uses a *branch name* — `main`, `master`, `develop`,
`my-feature` — is rejected because the file at that path will change
without warning when the branch advances. The rule applies to the
common forges:

- GitHub: `https://github.com/{owner}/{repo}/(blob|tree|raw|edit|commit)/{ref}/{path}`
- GitLab (gitlab.com or self-hosted): `https://gitlab.com/{owner}/{repo}/-/(blob|tree|raw|commit|edit)/{ref}/{path}`
- Bitbucket: `https://bitbucket.org/{owner}/{repo}/src/{ref}/{path}`
- Codeberg / Gitea / Forgejo: `https://codeberg.org/{owner}/{repo}/src/(branch|commit|tag)/{ref}/{path}` — the URL itself names the ref kind, which the lint uses directly.
- sourcehut: `https://git.sr.ht/~{user}/{repo}/tree/{ref}/item/{path}`
- gitee: same shape as GitHub.

The pattern for each forge is configurable; new self-hosted instances
only need a host plus a path-template addition.

## What to lint

For every URL that matches one of the configured forge patterns,
extract the `{ref}` segment and classify it:

- **Long SHA** (`^[0-9a-fA-F]{40}$`): accept.
- **Short SHA** (`^[0-9a-fA-F]{N,39}$`, where `N` defaults to 7):
  accept.
- **Tag** (matches one of the configured `tag_patterns`, or appears
  in `tag_allowlist`): accept iff `allow_tags = true`.
- **Codeberg / Gitea ref-typed URL**: when the URL contains
  `/src/branch/`, the ref is *known* to be a branch and is rejected
  regardless of how it would otherwise classify; `/src/commit/` and
  `/src/tag/` are accepted (the latter subject to `allow_tags`).
- **Anything else** (typically a branch name): reject.

Diagnostic span is the `{ref}` substring of the URL, with help text
that explains what kind of ref the lint expected.

## Examples

```rust
// Bad: branch ref
/// See <https://github.com/owner/repo/blob/main/src/lib.rs>.

// Good: long SHA
/// See <https://github.com/owner/repo/blob/8c1f6e2a6d33c1b1a2f9e0e1d3b8a4c7d6e5f4a3/src/lib.rs>.

// Good: short SHA (7+ hex chars)
/// See <https://github.com/owner/repo/blob/8c1f6e2/src/lib.rs>.

// Good (default config): semver tag
/// See <https://github.com/owner/repo/blob/v1.2.3/src/lib.rs>.

// Bad on Codeberg: explicit `/src/branch/`
/// See <https://codeberg.org/owner/repo/src/branch/main/src/lib.rs>.

// Good on Codeberg: explicit `/src/commit/`
/// See <https://codeberg.org/owner/repo/src/commit/8c1f6e2.../src/lib.rs>.
```

## Configuration

```toml
[unpinned_repo_ref]
# Where the lint scans. Subset of these.
targets = ["doc", "comment", "string_literal"]

# Are tag refs accepted as pinned?
allow_tags = true

# Patterns that classify a ref as a tag rather than a branch. The
# defaults catch semver and date-based releases.
tag_patterns = [
  "^v?[0-9]+(\\.[0-9]+){0,2}([-+][0-9A-Za-z.-]+)?$",   # v1.2.3, 1.2, 1.2.3-rc1
  "^release[-/][0-9].*",                                # release-2024-01
  "^[0-9]{4}-[0-9]{2}-[0-9]{2}$",                       # 2024-01-15
]

# Exact strings always accepted as tags, regardless of pattern match.
tag_allowlist = []

# Minimum length for a hex string to count as a "short SHA".
min_short_commit_length = 7

# Forge URL patterns. Each entry maps a host glob to the path
# template that locates the ref segment. `{ref}` is the capture
# point; `**` matches any path suffix.
forges = [
  { host = "github.com",     paths = ["{owner}/{repo}/blob/{ref}/**",
                                       "{owner}/{repo}/tree/{ref}/**",
                                       "{owner}/{repo}/raw/{ref}/**",
                                       "{owner}/{repo}/edit/{ref}/**",
                                       "{owner}/{repo}/commit/{ref}",
                                       "{owner}/{repo}/blame/{ref}/**"] },
  { host = "gitlab.com",     paths = ["{owner}/{repo}/-/blob/{ref}/**",
                                       "{owner}/{repo}/-/tree/{ref}/**",
                                       "{owner}/{repo}/-/raw/{ref}/**",
                                       "{owner}/{repo}/-/edit/{ref}/**",
                                       "{owner}/{repo}/-/commit/{ref}"] },
  { host = "bitbucket.org",  paths = ["{owner}/{repo}/src/{ref}/**"] },
  { host = "codeberg.org",   paths = ["{owner}/{repo}/src/branch/{ref}/**",
                                       "{owner}/{repo}/src/commit/{ref}/**",
                                       "{owner}/{repo}/src/tag/{ref}/**",
                                       "{owner}/{repo}/raw/branch/{ref}/**",
                                       "{owner}/{repo}/raw/commit/{ref}/**"] },
  { host = "gitee.com",      paths = ["{owner}/{repo}/blob/{ref}/**",
                                       "{owner}/{repo}/tree/{ref}/**"] },
  { host = "git.sr.ht",      paths = ["~{user}/{repo}/tree/{ref}/item/**",
                                       "~{user}/{repo}/blob/{ref}/**"] },
]

# Hosts whose `/src/branch/` style URLs are inherently branch refs
# regardless of how the ref would otherwise classify. Codeberg, Gitea,
# and Forgejo all share this convention.
ref_typed_hosts = ["codeberg.org"]

# Skip URLs whose host matches one of these glob patterns. Useful for
# placeholder hosts and for projects that vendor their own repository
# under a path that should remain branch-pinned.
skip_hosts = []
```

A self-hosted GitLab is registered by adding an entry to `forges`
with the same `paths` template and the new host. The `host` field
accepts a glob (`gitlab.*.example.com`) so subdomains can be covered
in one entry.

## Implementation notes

- `LateLintPass`. Reuse the URL discovery logic from
  [`bare-url`](./bare-url.md) — same scanner, same code-span / code-
  block exclusions, same retokenization for regular comments. The
  same URL match feeds both lints.
- Parse each candidate URL with the standard library's `Url`-equivalent
  via `url::Url` (an existing dependency of clippy_utils' ecosystem)
  or a small hand-written parser. Match `host` against the configured
  glob entries; for each match, walk the path components against the
  template's `{owner}/{repo}/.../{ref}/**` pattern and extract the
  ref.
- Ref classification:
  1. Long SHA: pure hex, length 40.
  2. Short SHA: pure hex, length in `[min_short_commit_length, 39]`.
  3. Codeberg/Gitea ref-typed URL: classification is forced by the
     URL path (`/src/branch/` → branch regardless of contents).
  4. Tag: regex match against `tag_patterns`, or membership in
     `tag_allowlist`.
  5. Otherwise: branch.
- Diagnostic span pinpoints the ref substring, with a help message
  recommending the pinned form. **No autofix.** A Dylint pass cannot
  resolve a branch to a SHA without shelling out to `git` or the
  forge's API; both would be wildly out of scope and would also be
  pointing at the wrong thing (the URL might reference a *different*
  repository than the local one).

## Severity

Warn by default. Projects that follow pacquet's "cardinal rule"
posture should set this to deny in CI.

## Interaction with `bare-url`

The two lints layer cleanly:

1. `bare-url` ensures the URL is wrapped — `<...>` or `[label](...)`.
2. `unpinned_repo_ref` ensures the wrapped URL is pinned.

A URL that fails both rules produces two diagnostics, one per
concern. Neither lint suppresses the other.
