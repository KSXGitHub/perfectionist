use super::check_against;
use std::ffi::OsStr;
use std::path::Path;

/// A directory for a case to state and to run under, standing in for
/// whatever the `test` recipe chooses.
const CHOSEN_TMPDIR: &str = "/tmp/perfectionist-tests";

/// Require [`check_against`] to accept `temp_dir` against `expected`,
/// failing with its refusal where it does not.
fn accept(temp_dir: &str, expected: &str) {
    if let Err(error) = check_against(Path::new(temp_dir), Some(OsStr::new(expected))) {
        panic!("{error}");
    }
}

/// The message [`check_against`] refuses `temp_dir` and `expected`
/// with.
fn refusal(temp_dir: &str, expected: Option<&str>) -> String {
    check_against(Path::new(temp_dir), expected.map(OsStr::new))
        .expect_err("these arguments are a refusal")
        .to_string()
}

#[test]
fn the_recipes_directory_is_accepted() {
    accept(CHOSEN_TMPDIR, CHOSEN_TMPDIR);
}

/// A `Path` compares by component, so a trailing separator on one
/// side of the comparison is a spelling rather than a mismatch.
#[test]
fn a_trailing_separator_is_not_a_mismatch() {
    accept(CHOSEN_TMPDIR, "/tmp/perfectionist-tests/");
}

/// What a bare `cargo test` reaches: whatever `TMPDIR` says, and
/// nothing stating what the `.stderr` fixtures were recorded under.
#[test]
fn a_run_the_recipe_did_not_set_up_is_refused() {
    assert_eq!(
        refusal("/tmp", None),
        "PERFECTIONIST_TEST_TMPDIR is unset, so nothing says which temp directory the \
         `.stderr` fixtures were recorded under; this run's is \"/tmp\". Run `just test`.",
    );
}

#[test]
fn an_empty_value_is_read_as_unset() {
    assert_eq!(refusal("/tmp", Some("")), refusal("/tmp", None));
}

#[test]
fn a_temp_directory_other_than_the_recipes_is_refused() {
    assert_eq!(
        refusal("/tmp", Some(CHOSEN_TMPDIR)),
        "this run's temp directory is \"/tmp\", but PERFECTIONIST_TEST_TMPDIR says the \
         `.stderr` fixtures were recorded under \"/tmp/perfectionist-tests\".",
    );
}
