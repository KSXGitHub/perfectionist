// `allow_release_tags = true` (see `tests/unpinned_repo_ref.rs`)
// accepts release-shaped tag refs. It does not disable the rule:
// ordinary branch refs are still flagged.

#![allow(unknown_lints, reason = "ui fixture")]

/// Good: configured release-like tag ref:
/// <https://github.com/owner/repo/blob/v1.2.3/src/lib.rs>.
fn _doc_github_release_tag() {}

// Good: configured gitea explicit release-like tag:
// <https://codeberg.org/owner/repo/src/tag/v1.2.3-rc.1/src/lib.rs>
fn _comment_gitea_release_tag() {}

/// Bad: branch ref still requires a commit SHA:
/// <https://github.com/owner/repo/blob/main/src/lib.rs>.
fn _doc_github_branch() {}

fn main() {}
