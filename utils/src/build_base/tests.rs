use super::check_against;
use std::ffi::OsStr;
use std::path::Path;

/// The directory the `test` recipe chooses, as both the recipe and
/// the process see it.
const RECIPE_TMPDIR: &str = "/tmp/perfectionist-tests";

#[test]
fn the_recipes_directory_is_accepted() {
    check_against(Path::new(RECIPE_TMPDIR), Some(OsStr::new(RECIPE_TMPDIR)));
}

/// A `Path` compares by component, so a trailing separator on one
/// side of the comparison is a spelling rather than a mismatch.
#[test]
fn a_trailing_separator_is_not_a_mismatch() {
    check_against(
        Path::new(RECIPE_TMPDIR),
        Some(OsStr::new("/tmp/perfectionist-tests/")),
    );
}

/// What a bare `cargo test` reaches: whatever `TMPDIR` says, and
/// nothing stating what the fixtures were recorded under.
#[test]
#[should_panic(expected = "is unset, so nothing says which temp directory")]
fn a_run_the_recipe_did_not_set_up_is_refused() {
    check_against(Path::new("/tmp"), None);
}

/// An empty value is what a recipe forwarding an unset variable
/// passes on, so it is read as the variable's absence.
#[test]
#[should_panic(expected = "is unset, so nothing says which temp directory")]
fn an_empty_value_is_read_as_unset() {
    check_against(Path::new("/tmp"), Some(OsStr::new("")));
}

#[test]
#[should_panic(expected = "says the suite was set up with")]
fn a_temp_directory_other_than_the_recipes_is_refused() {
    check_against(Path::new("/tmp"), Some(OsStr::new(RECIPE_TMPDIR)));
}
