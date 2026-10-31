//! Hold a run to the temp directory the `test` recipe chose.
//!
//! `dylint_testing` leaves compiletest's output directory at its
//! default, [`env::temp_dir`], and compiletest substitutes
//! `$TEST_BUILD_DIR` for that path throughout a fixture's rendered
//! output. The `test` recipe chooses the directory and states the
//! choice in [`EXPECTED_VAR`].

use derive_more::Display;
use std::env;
use std::ffi::OsStr;
use std::path::Path;

/// Where the `test` recipe states the temp directory it chose.
const EXPECTED_VAR: &str = "PERFECTIONIST_TEST_TMPDIR";

pub(crate) fn check() {
    let temp_dir = env::temp_dir();
    let expected = env::var_os(EXPECTED_VAR);
    if let Err(error) = check_against(&temp_dir, expected.as_deref()) {
        panic!("{error}");
    }
}

/// [`check`] with the temp directory and the recipe's value given
/// rather than read from the environment.
fn check_against<'a>(
    temp_dir: &'a Path,
    expected: Option<&'a OsStr>,
) -> Result<(), TempDirError<'a>> {
    let expected = expected.ok_or(TempDirError::Unstated { actual: temp_dir })?;
    if temp_dir == Path::new(expected) {
        return Ok(());
    }
    Err(TempDirError::Mismatched {
        actual: temp_dir,
        stated: expected,
    })
}

/// Why a run's temp directory cannot be taken for the one the `test`
/// recipe chose.
#[derive(Debug, Display, PartialEq)]
enum TempDirError<'a> {
    #[display(
        "{EXPECTED_VAR} is unset, so nothing says which temp directory the `.stderr` fixtures \
         were recorded under; this run's is {actual:?}. Run `just test`."
    )]
    Unstated { actual: &'a Path },
    #[display(
        "this run's temp directory is {actual:?}, but {EXPECTED_VAR} says the `.stderr` \
         fixtures were recorded under {stated:?}."
    )]
    Mismatched { actual: &'a Path, stated: &'a OsStr },
}

#[cfg(test)]
mod tests;
