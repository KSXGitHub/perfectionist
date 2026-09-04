// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// `skip_hosts = ["*.internal.example", "GITHUB.COM"]` (see
// `tests/unpinned_repo_ref.rs`) ignores matching hosts even when they
// are in the `hosts` table. Patterns are `*`-globs compared
// case-insensitively.

#![allow(unknown_lints, reason = "ui fixture")]

/// Skipped by the `*.internal.example` glob:
/// <https://git.internal.example/group/repo/-/blob/main/src/lib.rs>.
fn _doc_glob_skip() {}

/// Skipped by `GITHUB.COM` despite the case difference, even though
/// `github.com` is in the built-in host table:
/// <https://github.com/owner/repo/blob/main/src/lib.rs>.
fn _doc_case_insensitive_skip() {}

/// Bad: `gitlab.com` matches no skip pattern, so the built-in host
/// table still flags this unpinned branch ref:
/// <https://gitlab.com/group/repo/-/blob/main/src/lib.rs>.
fn _doc_not_skipped() {}

fn main() {}
