//! Drive every codebase fixture under `tests/fixtures/autofix/`
//! through the rounds of autofixes it takes to settle.
//!
//! One test per codebase fixture, not one per settling path: the
//! paths over one codebase share a build cache, and two of them
//! running at once would have cargo rebuild the fixture's members
//! back and forth between the two trees. A path that fails does not
//! stop the next one, so one run reports every path that came out
//! short.
//!
//! The harness itself is `_utils::autofix`, whose module
//! documentation explains what a round, a settled codebase and a
//! settling path are, and what each round is held to. Re-record every
//! expectation with `PERFECTIONIST_BLESS_AUTOFIX=1`, then read the
//! diff of the fixture files as the review of the autofixes.

pub mod _utils;

use _utils::autofix::check_codebase;

/// The codebase with no registry dependencies, under the default
/// configuration and under the one that opts into the rules shipping
/// inactive.
#[test]
fn plain() {
    check_codebase(_utils::cargo_manifest_dir(), "plain");
}

/// The codebase of shapes the rules report and deliberately decline
/// to hand the fixer, with no registry dependencies.
#[test]
fn plain_declined() {
    check_codebase(_utils::cargo_manifest_dir(), "plain-declined");
}

/// The codebase whose rules need a registry crate present, either to
/// fire at all or to have somewhere to rewrite a call to.
#[test]
fn third_party() {
    check_codebase(_utils::cargo_manifest_dir(), "third-party");
}

/// The codebase of shapes the rules report and deliberately decline
/// to hand the fixer, among the rules that need a registry crate.
#[test]
fn third_party_declined() {
    check_codebase(_utils::cargo_manifest_dir(), "third-party-declined");
}
