use super::{TempDirError, check_against};
use std::ffi::OsStr;
use std::path::Path;

/// A directory for a case to state and to run under, standing in for
/// whatever the `test` recipe chooses.
const CHOSEN_TMPDIR: &str = "/tmp/perfectionist-tests";

#[test]
fn the_recipes_directory_is_accepted() {
    check_against(Path::new(CHOSEN_TMPDIR), Some(OsStr::new(CHOSEN_TMPDIR)))
        .expect("the stated directory is this run's");
}

/// A `Path` compares by component, so a trailing separator on one
/// side of the comparison is a spelling rather than a mismatch.
#[test]
fn a_trailing_separator_is_not_a_mismatch() {
    check_against(
        Path::new(CHOSEN_TMPDIR),
        Some(OsStr::new("/tmp/perfectionist-tests/")),
    )
    .expect("a trailing separator is a spelling");
}

/// What a bare `cargo test` reaches: whatever `TMPDIR` says, and
/// nothing stating what the `.stderr` fixtures were recorded under.
#[test]
fn a_run_the_recipe_did_not_set_up_is_refused() {
    assert_eq!(
        check_against(Path::new("/tmp"), None),
        Err(TempDirError::Unstated {
            actual: Path::new("/tmp"),
        }),
    );
}

#[test]
fn a_temp_directory_other_than_the_recipes_is_refused() {
    assert_eq!(
        check_against(Path::new("/tmp"), Some(OsStr::new(CHOSEN_TMPDIR))),
        Err(TempDirError::Mismatched {
            actual: Path::new("/tmp"),
            stated: OsStr::new(CHOSEN_TMPDIR),
        }),
    );
}
