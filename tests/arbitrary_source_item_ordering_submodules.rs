//! Integration tests for `arbitrary_source_item_ordering` against real
//! on-disk Cargo projects, covering the two things a single-file UI
//! fixture cannot:
//!
//! - A module body written in a separate-file `mod foo;` submodule,
//!   which a pre-expansion `EarlyLintPass` would skip entirely (the
//!   file is `ModKind::Unloaded` until macro expansion). The rule runs
//!   as a late pass that re-parses every module source file, so each
//!   separate-file submodule is covered — including one reached only
//!   through another.
//! - A `#[cfg(test)] mod tests { ... }` body, which is part of the
//!   crate only under the unit-test target. Re-parsing keeps
//!   cfg-disabled modules, so the rule must consult `live_module_spans`
//!   before descending into an inline module or it lints code that is
//!   not in the build.
//!
//! These materialise a project on disk and run `cargo dylint`, sharing
//! the warmed `target/integration-fixtures`. The rule is active by
//! default, so no `dylint.toml` is needed to switch it on.

pub mod _utils;

use _utils::{
    cargo_manifest_dir, run_project_with_config, run_project_with_sources, shared_target_dir,
};

const LINT: &str = "perfectionist::arbitrary_source_item_ordering";

const CFG_TEST_LIB: &str =
    include_str!("fixtures/arbitrary_source_item_ordering_submodules/cfg_test_lib.rs");

const CFG_TEST_SEPARATE: &str =
    include_str!("fixtures/arbitrary_source_item_ordering_submodules/cfg_test_separate.rs");

/// The rule flags a misordered module body wherever it is written: in
/// the crate root, in an inline `mod { ... }`, in a separate-file
/// `mod foo;` submodule, and in one nested under another separate file.
#[test]
fn flags_misordered_items_in_separate_file_submodules() {
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_asio_separate_module",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            (
                "src/lib.rs",
                include_str!("fixtures/arbitrary_source_item_ordering_submodules/flags_lib.rs"),
            ),
            (
                "src/separate.rs",
                include_str!("fixtures/arbitrary_source_item_ordering_submodules/separate.rs"),
            ),
            (
                "src/separate/deep.rs",
                include_str!("fixtures/arbitrary_source_item_ordering_submodules/deep.rs"),
            ),
        ],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains(LINT),
        "expected `{LINT}` warnings; stderr was:\n{stderr}",
    );
    // The regression: the misordered body inside the separate-file
    // submodule is reported at `src/separate.rs` rather than skipped...
    assert!(
        stderr.contains("src/separate.rs"),
        "expected the separate-file submodule to be flagged; stderr was:\n{stderr}",
    );
    // ...including one nested under another separate-file module.
    assert!(
        stderr.contains("src/separate/deep.rs"),
        "expected the nested separate-file submodule to be flagged; stderr was:\n{stderr}",
    );
    // The crate root and the inline module stay covered.
    assert!(
        stderr.contains("src/lib.rs"),
        "expected the crate-root and inline-module forms to stay flagged; \
         stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains("pub mod nested"),
        "expected the inline module's misplaced `pub mod` to be flagged; \
         stderr was:\n{stderr}",
    );
}

/// Under `--all-targets` the unit-test target is compiled, so the
/// `#[cfg(test)] mod tests { ... }` body is live and its misordered
/// `pub mod` is flagged.
#[test]
fn flags_cfg_test_module_under_all_targets() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_asio_cfg_test",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", CFG_TEST_LIB),
            ("src/separate.rs", CFG_TEST_SEPARATE),
        ],
        // Default configuration: the rule is active out of the box.
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("pub mod helpers"),
        "expected the `#[cfg(test)]` module's misplaced `pub mod` to be flagged; \
         stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains("src/separate.rs"),
        "expected the separate-file submodule to be flagged; stderr was:\n{stderr}",
    );
}

/// Without `--all-targets` only the library target is compiled, so the
/// `#[cfg(test)] mod tests` is cfg-stripped and *not* part of the crate.
/// The `live_module_spans` guard must keep the rule from descending into
/// that re-parsed-but-dead inline module, while the separate-file
/// violation (genuinely part of the lib) is still flagged.
#[test]
fn cfg_test_module_skipped_in_library_build() {
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_asio_lib_only",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", CFG_TEST_LIB),
            ("src/separate.rs", CFG_TEST_SEPARATE),
        ],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    // The rule still runs: the separate-file violation is flagged.
    assert!(
        stderr.contains("src/separate.rs"),
        "expected the separate-file submodule to be flagged; stderr was:\n{stderr}",
    );
    // But the cfg-disabled module is not part of the compiled library, so
    // it must not be flagged (and, having no HIR node, it could not be
    // suppressed by a local `#[allow]` if it were).
    assert!(
        !stderr.contains("pub mod helpers"),
        "the `#[cfg(test)]` module is not in the library build and must not be \
         flagged; stderr was:\n{stderr}",
    );
}

/// An `#[allow]` on the out-of-line `mod separate;` declaration
/// suppresses the rule inside the submodule's own file: anchoring each
/// violation at its enclosing HIR node lets the module-level
/// suppression resolve. The crate-root control violation must still
/// fire, so the test proves the suppression is specific rather than the
/// rule silently skipping the separate file.
#[test]
fn respects_allow_on_separate_file_submodule() {
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_asio_allowed",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            (
                "src/lib.rs",
                include_str!("fixtures/arbitrary_source_item_ordering_submodules/allowed_lib.rs"),
            ),
            (
                "src/separate.rs",
                include_str!("fixtures/arbitrary_source_item_ordering_submodules/separate.rs"),
            ),
            (
                "src/separate/deep.rs",
                include_str!("fixtures/arbitrary_source_item_ordering_submodules/deep.rs"),
            ),
        ],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    // The control crate-root violation IS flagged, proving the rule runs...
    assert!(
        stderr.contains(LINT) && stderr.contains("src/lib.rs"),
        "expected the control crate-root violation to be flagged; \
         stderr was:\n{stderr}",
    );
    // ...and the `#[allow]` on `mod separate;` specifically suppresses the
    // separate file's findings, including the one in the file it declares.
    assert!(
        !stderr.contains("src/separate.rs"),
        "expected the `#[allow]` on `mod separate;` to suppress the \
         separate file's finding; stderr was:\n{stderr}",
    );
    assert!(
        !stderr.contains("src/separate/deep.rs"),
        "expected the `#[allow]` on `mod separate;` to reach the module \
         nested under it; stderr was:\n{stderr}",
    );
}
