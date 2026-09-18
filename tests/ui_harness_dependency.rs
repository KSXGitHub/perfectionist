//! Guard the property that makes `_utils::configured_ui_test` the only
//! route to a configured UI test: this package does not depend on the
//! UI harness, so no integration test here can name the builder that
//! reaches `DYLINT_TOML` directly.
//!
//! While the dependency is absent the compiler enforces that on its
//! own. This test covers the step that would quietly undo it — the
//! dependency restored to a manifest, say by a merge resolved against
//! a branch predating its removal — and says why it was moved, where
//! the compiler would only stop reporting an unknown crate.

use std::fs;
use std::path::Path;

/// The UI harness, which reaches its driver through a process-global
/// environment variable. `_utils` depends on it and wraps that access
/// in a helper that takes a lock first; this package must not, or a
/// test binary could name the builder and skip the lock.
const UI_HARNESS: &str = "dylint_testing";

/// The manifest tables whose entries put a crate in this package's
/// extern prelude, and so would make [`UI_HARNESS`] nameable from
/// `tests/`.
const DEPENDENCY_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

#[test]
fn the_lint_crate_does_not_depend_on_the_ui_harness() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text = fs::read_to_string(&path).expect("read this package's Cargo.toml");
    let manifest: toml::Table = toml::from_str(&text).expect("parse this package's Cargo.toml");
    let mut offenders = Vec::new();
    for table in DEPENDENCY_TABLES {
        let Some(entries) = manifest.get(*table).and_then(toml::Value::as_table) else {
            continue;
        };
        for (name, spec) in entries {
            if name == UI_HARNESS || renames_the_ui_harness(spec) {
                offenders.push(format!("[{table}] {name}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "this package depends on `{UI_HARNESS}`: {offenders:?}\n\
         The dependency belongs to `_utils`, whose `configured_ui_test` holds the lock that \
         keeps one fixture's `DYLINT_TOML` out of another fixture's run. While it is absent \
         here, an integration test that names the harness does not compile.",
    );
}

/// Whether a dependency entry pulls in [`UI_HARNESS`] under another
/// name, which its own key then hides.
fn renames_the_ui_harness(spec: &toml::Value) -> bool {
    spec.get("package").and_then(toml::Value::as_str) == Some(UI_HARNESS)
}
