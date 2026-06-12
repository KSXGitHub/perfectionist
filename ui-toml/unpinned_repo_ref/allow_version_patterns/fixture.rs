// `allow_version_patterns = true` (see `tests/unpinned_repo_ref.rs`)
// accepts version-shaped refs. It does not disable the rule:
// ordinary branch refs are still flagged.

#![allow(unknown_lints, reason = "ui fixture")]

/// Good: configured version-pattern ref:
/// <https://github.com/owner/repo/blob/v1.2.3/src/lib.rs>.
fn _doc_github_version_pattern() {}

// Good: configured gitea explicit version-pattern ref:
// <https://codeberg.org/owner/repo/src/tag/v1.2.3-rc.1/src/lib.rs>
fn _comment_gitea_version_pattern() {}

/// Bad: branch ref still requires a commit SHA:
/// <https://github.com/owner/repo/blob/main/src/lib.rs>.
fn _doc_github_branch() {}

fn main() {}
