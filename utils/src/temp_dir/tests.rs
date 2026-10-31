use super::{TempDirError, check_against};
use pipe_trait::Pipe;
use std::path::Path;

#[test]
fn the_directory_just_test_chooses_is_accepted() {
    "/tmp/perfectionist-tests"
        .pipe_as_ref(check_against)
        .expect("a chosen temp directory");
}

/// Only the last component is read, because the path above it is
/// whatever `TMPDIR` already held and differs between machines.
#[test]
fn the_path_above_the_chosen_directory_may_be_anything() {
    "/var/folders/a1/b2/T/perfectionist-tests"
        .pipe_as_ref(check_against)
        .expect("a chosen temp directory below another root");
}

/// `Path::file_name` reads the last component whether or not a
/// separator follows it, so the spelling of one is not a refusal.
#[test]
fn a_trailing_separator_is_accepted() {
    "/tmp/perfectionist-tests/"
        .pipe_as_ref(check_against)
        .expect("a chosen temp directory");
}

/// What a bare `cargo test` reaches: whatever `TMPDIR` says, which is
/// not where the `.stderr` fixtures were recorded.
#[test]
fn a_run_just_test_did_not_set_up_is_refused() {
    assert_eq!(
        check_against(Path::new("/tmp")),
        Err(TempDirError {
            actual: Path::new("/tmp"),
        }),
    );
}
