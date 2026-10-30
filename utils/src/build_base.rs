//! The directory compiletest writes a run's output to.
//!
//! `dylint_testing` leaves it at compiletest's default,
//! [`env::temp_dir`], and compiletest substitutes `$TEST_BUILD_DIR`
//! for its path throughout a fixture's rendered output, the echoed
//! source lines included. A fixture that names a temp directory is
//! therefore judged on text `TMPDIR` decides, and `ui/` holds such a
//! fixture.
//!
//! Nothing here can choose the directory: `dylint_testing::ui::Test`
//! exposes no `build_base` setter, and setting `TMPDIR` in-process is
//! unsafe in edition 2024. The `test` recipe chooses it and states the
//! choice in [`EXPECTED_VAR`].

use derive_more::Display;
use std::env;
use std::ffi::OsStr;
use std::path::Path;

/// Where the `test` recipe states the temp directory it chose.
const EXPECTED_VAR: &str = "PERFECTIONIST_TEST_TMPDIR";

/// Refuse a run whose temp directory is not the one the `test` recipe
/// chose.
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
    let Some(expected) = expected else {
        return Err(TempDirError::Unstated { actual: temp_dir });
    };
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
