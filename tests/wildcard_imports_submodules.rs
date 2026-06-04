//! Integration tests for `wildcard_imports` against real on-disk Cargo
//! projects, covering the two things a single-file UI fixture cannot:
//!
//! - The rule's headline case, `#[cfg(test)] mod tests { use super::*; }`,
//!   is cfg-gated. It is only part of the compiled crate under the
//!   unit-test target, so it is reachable only by re-parsing each module
//!   file in a late pass while consulting `live_module_spans` — the same
//!   trap the sibling `import_grouping` rule documents.
//! - A glob in a separate-file `mod foo;` submodule, which a
//!   pre-expansion `EarlyLintPass` would skip entirely (the file is
//!   `ModKind::Unloaded` until macro expansion).
//!
//! These materialise a project on disk and run `cargo dylint`, sharing
//! the warmed `target/integration-fixtures`.

pub mod _utils;

use _utils::{
    cargo_manifest_dir, run_project_with_config, run_project_with_sources, shared_target_dir,
};

const LINT: &str = "perfectionist::wildcard_imports";

const LIB_WITH_CFG_TEST: &str = "\
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, unused_imports, dead_code, reason = \"fixture\")]

pub struct Thing;

mod separate;

#[cfg(test)]
mod tests {
    use super::*;
}
";

const SEPARATE_GLOB: &str = "use std::collections::*;\n";

/// Under `--all-targets` the unit-test target is compiled, so the
/// `#[cfg(test)] mod tests { use super::*; }` is live and flagged — and
/// the glob in the separate-file submodule is flagged too.
#[test]
fn flags_cfg_test_glob_and_separate_file() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_wi_cfg_test",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", LIB_WITH_CFG_TEST),
            ("src/separate.rs", SEPARATE_GLOB),
        ],
        // Default configuration.
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains(LINT),
        "expected `{LINT}` warnings; stderr was:\n{stderr}",
    );
    // The cfg-gated `use super::*` is flagged once the test target makes
    // it live.
    assert!(
        stderr.contains("use super::*"),
        "expected the `#[cfg(test)]` glob to be flagged; stderr was:\n{stderr}",
    );
    // The separate-file submodule glob is reached by the re-parse.
    assert!(
        stderr.contains("src/separate.rs"),
        "expected the separate-file submodule glob to be flagged; stderr was:\n{stderr}",
    );
}

/// Without `--all-targets` only the library target is compiled, so the
/// `#[cfg(test)] mod tests` is cfg-stripped and *not* part of the crate.
/// The `live_module_spans` guard must keep the rule from descending into
/// that re-parsed-but-dead inline module, while the separate-file glob
/// (genuinely part of the lib) is still flagged.
#[test]
fn cfg_test_glob_skipped_in_library_build() {
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_wi_lib_only",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", LIB_WITH_CFG_TEST),
            ("src/separate.rs", SEPARATE_GLOB),
        ],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    // The rule still runs: the separate-file glob is flagged.
    assert!(
        stderr.contains("src/separate.rs"),
        "expected the separate-file submodule glob to be flagged; stderr was:\n{stderr}",
    );
    // But the cfg-disabled `use super::*` is not part of the compiled
    // library, so it must not be flagged (and, having no HIR node, it
    // could not be suppressed by a local `#[allow]` if it were).
    assert!(
        !stderr.contains("use super::*"),
        "the `#[cfg(test)]` glob is not in the library build and must not be flagged; \
         stderr was:\n{stderr}",
    );
}

/// An `#[allow]` on the out-of-line `mod separate;` declaration
/// suppresses the rule inside the submodule's own file: anchoring each
/// violation at its enclosing HIR node lets the module-level suppression
/// resolve.
#[test]
fn respects_allow_on_separate_file_submodule() {
    let lib = "\
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, unused_imports, dead_code, reason = \"fixture\")]

#[allow(perfectionist::wildcard_imports, reason = \"regression fixture\")]
mod separate;
";
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_wi_allowed",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[("src/lib.rs", lib), ("src/separate.rs", SEPARATE_GLOB)],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "expected the `#[allow]` on `mod separate;` to suppress the rule; \
         stderr was:\n{stderr}",
    );
}
