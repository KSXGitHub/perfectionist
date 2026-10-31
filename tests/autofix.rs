//! Drive every fixture under `tests/fixtures/autofix/` through the
//! rounds of autofixes it takes to settle.
//!
//! One test per fixture, not one per run: the runs over one fixture
//! share a build cache, and two of them going at once would have
//! cargo rebuild the project's members back and forth between the two
//! trees. A run that fails does not stop the next one, so one pass
//! reports every run that came out short.
//!
//! The harness itself is `_utils::autofix`, whose module
//! documentation explains what a round, a settled project and a run
//! are, and what each round is held to. Re-record every expectation
//! with `just bless`, then read the diff of the fixture files as the
//! review of the autofixes.

pub mod _utils;

use _utils::autofix::check_fixture;

/// The project with no registry dependencies, under the default
/// configuration and under the one that opts into the rules shipping
/// inactive.
#[test]
fn plain() {
    check_fixture(_utils::cargo_manifest_dir(), "plain");
}

/// The project whose rules need a registry crate present, either to
/// fire at all or to have somewhere to rewrite a call to.
#[test]
fn third_party() {
    check_fixture(_utils::cargo_manifest_dir(), "third-party");
}
