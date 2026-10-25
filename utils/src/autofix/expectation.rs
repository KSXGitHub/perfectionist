//! Hold a round to what the fixture stored for it, or re-record it.
//!
//! Every expectation this harness compares is a text file beside the
//! settling path's configuration, so reviewing a change to an autofix
//! is reading the diff of those files. Re-recording is opt-in through
//! an environment variable rather than a flag, because the harness is
//! driven by `cargo test` and a flag would have to cross the test
//! binary's own argument parsing.

use crate::autofix::report::BLESS;
use std::fs;
use std::path::Path;

/// Whether this run re-records the expectations instead of comparing
/// against them.
pub fn blessing() -> bool {
    std::env::var_os(BLESS).is_some_and(|value| !value.is_empty())
}

/// Compare `actual` against the text stored in `file`, or store it
/// where this run is [`blessing`]. `what` names the expectation in
/// the failure, and reads as the subject of "does not match".
pub fn check(file: &Path, actual: &str, what: &str) -> Result<(), String> {
    if blessing() {
        let parent = file.parent().expect("an expectation file has a parent");
        fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("create {}: {error}", parent.display()));
        fs::write(file, actual).unwrap_or_else(|error| panic!("write {}: {error}", file.display()));
        return Ok(());
    }
    let Ok(expected) = fs::read_to_string(file) else {
        return Err(format!(
            "{what} has no stored expectation at {}; record one with {BLESS}=1\n\
             what this run produced:\n{actual}",
            file.display(),
        ));
    };
    if expected == actual {
        return Ok(());
    }
    Err(format!(
        "{what} does not match {}\n\
         --- stored ---\n{expected}--- this run ---\n{actual}--- end ---",
        file.display(),
    ))
}
