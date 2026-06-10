// `hosts = []` (see `tests/unpinned_repo_ref.rs`) leaves the github
// host unrecognised, so the branch ref below is not flagged. No
// `.stderr` file: the fixture must produce no diagnostics.

#![allow(unknown_lints, reason = "ui fixture")]

/// A branch ref the default host table would flag, silent here:
/// <https://github.com/owner/repo/blob/main/src/lib.rs>.
fn _doc() {}

fn main() {}
