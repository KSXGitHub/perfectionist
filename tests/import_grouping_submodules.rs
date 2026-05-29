//! Regression test for the separate-file submodule gap (the same class
//! of bug reported for `import_granularity` in
//! <https://github.com/KSXGitHub/parallel-disk-usage/issues/431>): with
//! the rule running as a pre-expansion pass, out-of-line `mod foo;`
//! modules were still `ModKind::Unloaded` at lint time, so their `use`
//! statements were never inspected — only the crate-root file and inline
//! `mod { ... }` blocks were.
//!
//! These tests materialise a real Cargo project on disk (so the
//! separate-file module is loaded the way a normal build loads it) and
//! run `cargo dylint --all` against it, sharing the warmed
//! `target/integration-fixtures`. Pre-warm with
//! `just warmup-integration-tests`.
//!
//! The violation used throughout is a blank line splitting a single
//! `std` group: it is a violation under every `order` (so it does not
//! depend on the default group order) and the two imports come from
//! different modules, so `import_granularity`'s module style does not
//! also fire on them.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_sources, shared_target_dir};

const LINT: &str = "perfectionist::import_grouping";

/// A blank-line split inside a separate-file module (`mod foo;` →
/// `src/separate.rs`) is flagged, exactly as the identical split in the
/// crate root and in an inline module already is. The submodule nests a
/// further out-of-line module (`src/separate/deep.rs`) to cover modules
/// reached only through another separate-file module.
#[test]
fn flags_split_in_separate_file_submodule() {
    let lib = "\
#![allow(dead_code, unused_imports)]

use std::collections::BTreeMap;

use std::time::Duration;

mod separate;

mod inline {
    use std::vec::Vec;

    use std::rc::Rc;
}
";
    let separate = "\
use std::collections::BTreeMap;

use std::time::Duration;

mod deep;
";
    let deep = "\
use std::collections::BTreeMap;

use std::time::Duration;
";
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_igrp_separate_module",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", lib),
            ("src/separate.rs", separate),
            ("src/separate/deep.rs", deep),
        ],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains(LINT),
        "expected `{LINT}` warnings; stderr was:\n{stderr}",
    );
    // The regression: the split inside the separate-file submodule is now
    // reported at `src/separate.rs` rather than skipped...
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
        "expected the crate-root and inline-module splits to stay flagged; \
         stderr was:\n{stderr}",
    );
}

/// An `#[allow]` on the `mod foo;` declaration suppresses the rule inside
/// the submodule's own file: anchoring at the enclosing HIR node lets the
/// module-level suppression resolve.
#[test]
fn respects_allow_on_separate_file_submodule() {
    let lib = "\
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused_imports)]

#[allow(perfectionist::import_grouping, reason = \"regression fixture\")]
mod separate;
";
    let separate = "\
use std::collections::BTreeMap;

use std::time::Duration;
";
    let (_temp, stderr, success) = run_project_with_sources(
        "fixture_igrp_separate_module_allowed",
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
