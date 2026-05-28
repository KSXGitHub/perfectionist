//! Regression test for the separate-file submodule gap reported in
//! <https://github.com/KSXGitHub/parallel-disk-usage/issues/431>: with
//! the rule running as a pre-expansion pass, out-of-line `mod foo;`
//! modules were still `ModKind::Unloaded` at lint time, so their `use`
//! statements were never inspected — only the crate-root file and inline
//! `mod { ... }` blocks were.
//!
//! These tests materialise a real Cargo project on disk (so the
//! separate-file module is loaded the way a normal build loads it) and
//! run `cargo dylint --all` against it, sharing the warmed
//! `target/integration-fixtures`.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_sources, shared_target_dir};

const LINT: &str = "perfectionist::import_granularity";

/// The default `module` style flags a split that lives in a separate
/// file (`mod foo;` → `src/separate.rs`), exactly as it already flags
/// the identical split in the crate root and in an inline module.
#[test]
fn flags_split_in_separate_file_submodule() {
    let lib = "\
mod separate;

pub mod inline {
    use std::collections::BTreeMap;
    use std::collections::HashMap;

    pub fn touch() -> (BTreeMap<u8, u8>, HashMap<u8, u8>) {
        (BTreeMap::new(), HashMap::new())
    }
}

use std::collections::BTreeMap;
use std::collections::HashMap;

pub fn touch() -> (BTreeMap<u8, u8>, HashMap<u8, u8>) {
    (BTreeMap::new(), HashMap::new())
}
";
    let separate = "\
use std::collections::BTreeMap;
use std::collections::HashMap;

pub fn touch() -> (BTreeMap<u8, u8>, HashMap<u8, u8>) {
    (BTreeMap::new(), HashMap::new())
}
";
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_ig_separate_module",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[("src/lib.rs", lib), ("src/separate.rs", separate)],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains(LINT),
        "expected `{LINT}` warnings; stderr was:\n{stderr}",
    );
    // The regression: the split inside the separate-file submodule is now
    // reported, pointing at `src/separate.rs` rather than being skipped.
    assert!(
        stderr.contains("src/separate.rs"),
        "expected the separate-file submodule to be flagged; stderr was:\n{stderr}",
    );
    // The crate root and the inline module were already covered and must
    // stay covered.
    assert!(
        stderr.contains("src/lib.rs"),
        "expected the crate-root and inline-module splits to stay flagged; \
         stderr was:\n{stderr}",
    );
}

/// A separate-file submodule whose `#[allow]` sits on the `mod foo;`
/// declaration in the parent file is honoured: emitting at the enclosing
/// HIR node lets the module-level suppression resolve.
#[test]
fn respects_allow_on_separate_file_submodule() {
    let lib = "\
#![feature(register_tool)]
#![register_tool(perfectionist)]

#[allow(perfectionist::import_granularity, reason = \"regression fixture\")]
mod separate;
";
    let separate = "\
use std::collections::BTreeMap;
use std::collections::HashMap;

pub fn touch() -> (BTreeMap<u8, u8>, HashMap<u8, u8>) {
    (BTreeMap::new(), HashMap::new())
}
";
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_ig_separate_module_allowed",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[("src/lib.rs", lib), ("src/separate.rs", separate)],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "expected the `#[allow]` on `mod separate;` to suppress the rule; \
         stderr was:\n{stderr}",
    );
}
