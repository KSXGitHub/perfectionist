//! Regression test for the separate-file submodule gap: a source-layout
//! rule running as a pre-expansion pass would leave out-of-line
//! `mod foo;` modules `ModKind::Unloaded` at lint time, so their `use`
//! statements were never inspected — only the crate-root file and inline
//! `mod { ... }` blocks were. `uncombined_self_import` runs as a late pass
//! that re-parses every module source file, so each separate-file
//! submodule is covered.
//!
//! These tests materialise a real Cargo project on disk (so the
//! separate-file module is loaded the way a normal build loads it) and
//! run `cargo dylint --all` against it, sharing the warmed
//! `target/integration-fixtures`. `uncombined_self_import` is inactive by
//! default, so each test enables it through the appended `dylint.toml`
//! config (`import_granularity` is disabled so its own findings stay out
//! of the snapshot).

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};

const LINT: &str = "perfectionist::uncombined_self_import";

const CONFIG: &str = "\
[perfectionist]
enable = [\"uncombined_self_import\"]
disable = [\"import_granularity\"]
";

/// The rule folds an adjacent module + item import pair that lives in a
/// separate file (`mod foo;` → `src/separate.rs`), exactly as it already
/// flags the identical pair in the crate root and in an inline module.
/// The submodule nests a further out-of-line module
/// (`src/separate/deep.rs`) to cover modules reached only through another
/// separate-file module.
#[test]
fn flags_uncombined_self_import_in_separate_file_submodule() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_csi_separate_module",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            (
                "src/lib.rs",
                include_str!("fixtures/uncombined_self_import_submodules/flags_lib.rs"),
            ),
            (
                "src/separate.rs",
                include_str!("fixtures/uncombined_self_import_submodules/separate.rs"),
            ),
            (
                "src/separate/deep.rs",
                include_str!("fixtures/uncombined_self_import_submodules/deep.rs"),
            ),
        ],
        CONFIG,
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains(LINT),
        "expected `{LINT}` warnings; stderr was:\n{stderr}",
    );
    // The regression: the foldable pair inside the separate-file submodule
    // is now reported at `src/separate.rs` rather than skipped...
    assert!(
        stderr.contains("src/separate.rs"),
        "expected the separate-file submodule to be flagged; stderr was:\n{stderr}",
    );
    // ...including one nested under another separate-file module.
    assert!(
        stderr.contains("src/separate/deep.rs"),
        "expected the nested separate-file submodule to be flagged; stderr was:\n{stderr}",
    );
    // The crate root and inline module stay covered.
    assert!(
        stderr.contains("src/lib.rs"),
        "expected the crate-root and inline-module forms to stay flagged; \
         stderr was:\n{stderr}",
    );
}

/// An `#[allow]` on the `mod foo;` declaration suppresses the rule inside
/// the submodule's own file: anchoring each finding at its enclosing HIR
/// node lets the module-level suppression resolve. A crate-root control
/// pair (not under the `#[allow]`) must still fire, so the test proves
/// the suppression is specific rather than the rule silently skipping the
/// separate file.
#[test]
fn respects_allow_on_separate_file_submodule() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_csi_separate_module_allowed",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            (
                "src/lib.rs",
                include_str!("fixtures/uncombined_self_import_submodules/allowed_lib.rs"),
            ),
            (
                "src/separate.rs",
                include_str!("fixtures/uncombined_self_import_submodules/allowed_separate.rs"),
            ),
        ],
        CONFIG,
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    // The control crate-root pair IS flagged, proving the rule runs...
    assert!(
        stderr.contains(LINT) && stderr.contains("src/lib.rs"),
        "expected the control crate-root pair to be flagged; \
         stderr was:\n{stderr}",
    );
    // ...and the `#[allow]` on `mod separate;` specifically suppresses the
    // separate file's finding.
    assert!(
        !stderr.contains("src/separate.rs"),
        "expected the `#[allow]` on `mod separate;` to suppress the \
         separate file's finding; stderr was:\n{stderr}",
    );
}
