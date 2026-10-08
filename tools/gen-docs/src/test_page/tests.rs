//! What the JavaScript test page has to hold to.
//!
//! Its list of case files is written by hand, while the directory it
//! names grows on its own. This checks that the two still agree.

use super::{TEST_CASE_SCRIPTS, TEST_FIXTURE_SCRIPTS};
use std::fs;

#[test]
fn every_case_and_fixture_file_is_on_the_page() {
    // A file added to the directory is picked up by the headless runner
    // on its own, because that globs the directory. This page cannot
    // glob anything, so without this the two runners would quietly
    // disagree about what the suite is — and the browser, the one that
    // covers the older engines, would be the side running less.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests");
    let names = |suffix: &str| {
        let mut found: Vec<String> = fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("failed to read {dir}: {error}"))
            .map(|entry| entry.expect("failed to read a directory entry").file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| name.ends_with(suffix))
            .collect();
        found.sort();
        found
    };
    // Left unsorted, against a sorted directory listing, so the lists are
    // held to the order the headless runner reads the same files in —
    // which it gets from `readdir().sort()`. Membership alone would let
    // one fixture load after another that reads it in one runner and
    // before it in the other.
    let listed = |scripts: &[(&str, &str)]| {
        scripts
            .iter()
            .map(|&(name, _)| name.to_owned())
            .collect::<Vec<String>>()
    };
    assert_eq!(
        names(".test.js"),
        listed(TEST_CASE_SCRIPTS),
        "every *.test.js in {dir} must be listed in TEST_CASE_SCRIPTS, in that order",
    );
    assert_eq!(
        names(".fixtures.js"),
        listed(TEST_FIXTURE_SCRIPTS),
        "every *.fixtures.js in {dir} must be listed in TEST_FIXTURE_SCRIPTS, in that order",
    );
    assert!(!TEST_CASE_SCRIPTS.is_empty(), "the suite cannot be empty");
}
