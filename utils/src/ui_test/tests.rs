//! Guard the property that makes [`super::configured_ui_test`] the
//! repository's only route to a configured UI test: that no
//! integration test names the UI harness crate itself.
//!
//! The lint crate does not depend on `dylint_testing`, so a test file
//! that names it fails to compile already. This test covers the step
//! that would undo that — the dependency added back to a manifest —
//! and says why the helper exists, where the compiler would only
//! report an unknown crate.

use std::fs;
use std::path::{Path, PathBuf};

/// The directory this guard scans: the lint crate's integration
/// tests, which sit beside this crate rather than inside it.
fn integration_tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("this crate sits inside the repository")
        .join("tests")
}

#[test]
fn no_integration_test_names_the_ui_harness_crate() {
    let dir = integration_tests_dir();
    let mut scanned = 0_usize;
    let mut offenders = Vec::new();
    for entry in fs::read_dir(&dir).expect("read the integration-test directory") {
        let path = entry.expect("read an integration-test entry").path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        scanned += 1;
        let source = fs::read_to_string(&path).expect("read an integration test");
        if source.contains("dylint_testing") {
            offenders.push(path.file_name().unwrap_or_default().to_owned());
        }
    }
    assert!(scanned > 0, "no integration test was scanned in {dir:?}");
    assert!(
        offenders.is_empty(),
        "these integration tests reach the UI harness directly: {offenders:?}\n\
         Build the test through `_utils::configured_ui_test` instead, which \
         holds the lock that keeps one fixture's `DYLINT_TOML` out of another's run.",
    );
}
