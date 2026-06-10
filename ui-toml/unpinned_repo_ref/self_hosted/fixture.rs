// `hosts = [{ hostname = "git.example.com", kind = "gitlab" }]` (see
// `tests/unpinned_repo_ref.rs`) registers a self-hosted GitLab, so the
// gitlab-shaped branch URL below is flagged even though the host is
// not in the built-in table.

#![allow(unknown_lints, reason = "ui fixture")]

/// A branch ref on a self-hosted GitLab:
/// <https://git.example.com/group/repo/-/blob/main/src/lib.rs>.
fn _doc() {}

fn main() {}
