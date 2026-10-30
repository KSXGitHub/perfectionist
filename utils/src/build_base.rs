//! The directory compiletest writes a run's output to, and why the
//! suite requires the `test` recipe to have chosen it.
//!
//! `dylint_testing` leaves compiletest's `build_base` at compiletest's
//! own default, [`env::temp_dir`], so the directory is wherever
//! `TMPDIR` points. compiletest substitutes `$TEST_BUILD_DIR` for that
//! path throughout a fixture's rendered output, the echoed source
//! lines included, so a fixture whose own text holds the path is
//! judged on text it was not written as. `ui/` holds such a fixture
//! already, and a temp directory is a short, ordinary path that a
//! fixture about something else has every reason to name.
//!
//! The directory cannot be pinned from inside the process:
//! `dylint_testing::ui::Test` exposes no `build_base` setter, and
//! setting `TMPDIR` here is unsafe in edition 2024 and races the
//! threads a test binary runs its tests on. The `test` recipe chooses
//! it instead, and states what it chose in [`EXPECTED_VAR`], which is
//! what lets [`check`] hold the run to it. A run that reaches
//! compiletest under any other temp directory is one whose fixtures
//! are about to be diffed against `.stderr` files recorded under the
//! recipe's, and under `PERFECTIONIST_BLESS` it would record the
//! difference rather than report it.

use std::env;
use std::ffi::OsStr;
use std::path::Path;

/// Where the `test` recipe states the temp directory it chose.
const EXPECTED_VAR: &str = "PERFECTIONIST_TEST_TMPDIR";

/// Refuse a run whose temp directory is not the one the `test` recipe
/// chose.
pub(crate) fn check() {
    check_against(&env::temp_dir(), env::var_os(EXPECTED_VAR).as_deref());
}

/// [`check`] with the temp directory and the recipe's value given
/// rather than read from the environment.
fn check_against(temp_dir: &Path, expected: Option<&OsStr>) {
    // An empty value is what a recipe forwarding an unset variable
    // passes on, so it says no more than the variable's absence does.
    let expected = expected.filter(|value| !value.as_encoded_bytes().is_empty());
    let Some(expected) = expected else {
        panic!(
            "{EXPECTED_VAR} is unset, so nothing says which temp directory the fixtures' \
             `.stderr` files were recorded under; this run's is {temp_dir:?}. compiletest writes \
             a run's output to the temp directory and substitutes `$TEST_BUILD_DIR` for its \
             path throughout every fixture's rendered output, so a fixture that names a temp \
             directory is judged on different text under a different one. Run the suite through \
             `just test`, which chooses the directory and states it in {EXPECTED_VAR}. Reading \
             this from a `just test` run means the recipe and this check no longer name the same \
             variable.",
        );
    };
    assert!(
        temp_dir == Path::new(expected),
        "this run's temp directory is {temp_dir:?}, but {EXPECTED_VAR} says the suite was set up \
         with {expected:?}. compiletest substitutes `$TEST_BUILD_DIR` for the temp directory's \
         path throughout every fixture's rendered output, so a fixture that names a temp \
         directory would be diffed against text recorded under the other one. `TMPDIR` and \
         {EXPECTED_VAR} have to name the same directory.",
    );
}

#[cfg(test)]
mod tests;
