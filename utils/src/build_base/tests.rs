use super::check_against;
use std::ffi::OsStr;
use std::path::Path;

/// The directory the `test` recipe chooses, as both the recipe and
/// the process see it.
const RECIPE_TMPDIR: &str = "/tmp/perfectionist-tests";

/// Accept `temp_dir` against `expected`, reporting the refusal where
/// there is one.
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
    accept(RECIPE_TMPDIR, RECIPE_TMPDIR);
}

/// A `Path` compares by component, so a trailing separator on one
/// side of the comparison is a spelling rather than a mismatch.
#[test]
fn a_trailing_separator_is_not_a_mismatch() {
    accept(RECIPE_TMPDIR, "/tmp/perfectionist-tests/");
}

/// What a bare `cargo test` reaches: whatever `TMPDIR` says, and
/// nothing stating what the fixtures were recorded under.
#[test]
fn a_run_the_recipe_did_not_set_up_is_refused() {
    assert_eq!(
        refusal("/tmp", None),
        "PERFECTIONIST_TEST_TMPDIR is unset, so nothing says which temp directory the \
         `.stderr` fixtures were recorded under; this run's is \"/tmp\". Run `just test`.",
    );
}

/// An empty value is what a recipe forwarding an unset variable
/// passes on, so it is read as the variable's absence.
#[test]
fn an_empty_value_is_read_as_unset() {
    assert_eq!(refusal("/tmp", Some("")), refusal("/tmp", None));
}

#[test]
fn a_temp_directory_other_than_the_recipes_is_refused() {
    assert_eq!(
        refusal("/tmp", Some(RECIPE_TMPDIR)),
        "this run's temp directory is \"/tmp\", but PERFECTIONIST_TEST_TMPDIR says the \
         `.stderr` fixtures were recorded under \"/tmp/perfectionist-tests\".",
    );
}
