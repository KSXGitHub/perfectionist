//! Hold a run to the temp directory `just test` chooses.
//!
//! `dylint_testing` leaves compiletest's output directory at its
//! default, [`env::temp_dir`], and compiletest substitutes
//! `$TEST_BUILD_DIR` for that path throughout a fixture's rendered
//! output. `just test` points `TMPDIR` at a [`CHOSEN_NAME`] directory,
//! and that last component is what a run is recognised by: the path
//! above it is whatever `TMPDIR` already held.

use derive_more::Display;
use std::env;
use std::ffi::OsStr;
use std::path::Path;

/// The last component of the temp directory `just test` chooses.
const CHOSEN_NAME: &str = "perfectionist-tests";

pub(crate) fn check() {
    if let Err(error) = check_against(&env::temp_dir()) {
        panic!("{error}");
    }
}

/// [`check`] with the temp directory given rather than read from the
/// environment.
fn check_against(temp_dir: &Path) -> Result<(), TempDirError<'_>> {
    temp_dir
        .file_name()
        .filter(|name| *name == OsStr::new(CHOSEN_NAME))
        .ok_or(TempDirError { actual: temp_dir })
        .map(drop)
}

/// A temp directory `just test` did not choose.
#[derive(Debug, Display, PartialEq)]
#[display("this run's temp directory is {actual:?}, not a `{CHOSEN_NAME}` one. Run `just test`.")]
struct TempDirError<'a> {
    actual: &'a Path,
}

#[cfg(test)]
mod tests;
