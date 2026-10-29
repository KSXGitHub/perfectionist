//! Hold a round to what the fixture stored for it, or re-record it.
//!
//! Every expectation this harness compares is a text file beside the
//! run's configuration, so reviewing a change to an autofix is
//! reading the diff of those files.

use crate::bless;
use std::fs;
use std::path::Path;

/// Compare `actual` against the text stored in `file`, or store it
/// where this run re-records. `what` names the expectation in the
/// failure, and reads as the subject of "does not match".
pub fn check(file: &Path, actual: &str, what: &str) -> Result<(), String> {
    if *bless::ENABLED {
        let parent = file.parent().expect("an expectation file has a parent");
        fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("create {}: {error}", parent.display()));
        fs::write(file, actual).unwrap_or_else(|error| panic!("write {}: {error}", file.display()));
        return Ok(());
    }
    let Ok(expected) = fs::read_to_string(file) else {
        return Err(format!(
            "{what} has no stored expectation at {}; record one with `just bless`\n\
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
