// Default-config sweep for `unpinned_repo_ref`: every surface (doc
// comment, plain comment, string literal) and every built-in forge
// kind. Run by `tests/ui.rs` with an empty `dylint.toml`, so the
// defaults apply (all three targets, the built-in host table,
// `sha_recognition_length = 4`, `allow_version_patterns = false`, no
// skipped hosts).
//
// URLs are wrapped in `<...>` throughout: that is the realistic case
// (`perfectionist::bare_url` already requires wrapping, and this rule
// still pins the wrapped ref) and it keeps the fixture from also
// tripping `bare_url`.

#![allow(unknown_lints, reason = "ui fixture")]

// --- Doc comments ---

/// Bad: branch ref in a github blob URL:
/// <https://github.com/owner/repo/blob/main/src/lib.rs>.
fn _doc_github_branch() {}

/// Bad: tag ref (indistinguishable from a branch):
/// <https://github.com/owner/repo/blob/v1.2.3/src/lib.rs>.
fn _doc_github_tag() {}

/// Good: a commit SHA is pinned:
/// <https://github.com/owner/repo/blob/8c1f6e2a6d33c1b1a2f9e0e1d3b8a4c7d6e5f4a3/src/lib.rs>.
fn _doc_github_sha() {}

/// Good: a short SHA still counts as hex:
/// <https://github.com/owner/repo/blob/8c1f6e2/src/lib.rs>.
fn _doc_github_short_sha() {}

/// Bad: a 3-char hex ref is below `sha_recognition_length`:
/// <https://github.com/owner/repo/blob/abc/src/lib.rs>.
fn _doc_github_too_short() {}

/// Bad: gitee mirrors the github shape:
/// <https://gitee.com/owner/repo/blob/master/src/lib.rs>.
fn _doc_gitee_branch() {}

/// Inside a code span it is fine:
/// `https://github.com/owner/repo/blob/main/src/lib.rs`.
fn _doc_code_span_skip() {}

/// A non-forge host is ignored:
/// <https://example.com/owner/repo/blob/main/src/lib.rs>.
fn _doc_non_forge() {}

// --- Plain comments ---

// Bad: gitlab branch ref: <https://gitlab.com/group/repo/-/blob/main/x.rs>
fn _comment_gitlab_branch() {}

// Bad: gitlab subgroup branch ref:
// <https://gitlab.com/group/sub/repo/-/tree/develop/dir>
fn _comment_gitlab_subgroup() {}

// Bad: bitbucket branch ref: <https://bitbucket.org/owner/repo/src/master/f.rs>
fn _comment_bitbucket_branch() {}

// Bad: codeberg (gitea) explicit branch:
// <https://codeberg.org/owner/repo/src/branch/main/src/lib.rs>
fn _comment_gitea_branch() {}

// Bad: codeberg (gitea) explicit tag:
// <https://codeberg.org/owner/repo/src/tag/v1.2.3/src/lib.rs>
fn _comment_gitea_tag() {}

// Good: codeberg (gitea) explicit commit:
// <https://codeberg.org/owner/repo/src/commit/8c1f6e2/src/lib.rs>
fn _comment_gitea_commit() {}

// Bad: sourcehut branch ref: <https://git.sr.ht/~user/repo/tree/main/item/x.rs>
fn _comment_sourcehut_branch() {}

// --- String literals ---

fn _string_literals() {
    // Bad: branch ref in a string literal.
    let _bad = "https://github.com/owner/repo/blob/main/src/lib.rs";
    // Good: pinned to a commit SHA.
    let _good = "https://github.com/owner/repo/blob/8c1f6e2a6d33/src/lib.rs";
}

fn main() {}
