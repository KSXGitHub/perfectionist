// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// `scan_string_literals = true` (see `tests/unpinned_repo_ref.rs`)
// turns on the opt-in string-literal surface, which is off by default,
// so a branch ref in a string literal is flagged. A SHA-pinned literal
// is still accepted.

#![allow(unknown_lints, reason = "ui fixture")]

fn _string_literals() {
    // Bad: branch ref in a string literal, now scanned.
    let _bad = "https://github.com/owner/repo/blob/main/src/lib.rs";
    // Good: pinned to a commit SHA.
    let _good = "https://github.com/owner/repo/blob/8c1f6e2a6d33/src/lib.rs";
}

fn main() {}
