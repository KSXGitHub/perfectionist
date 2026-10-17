//! End-to-end proof that the `map_or_default` rewrite compiles where
//! `perfectionist::map_or` and `perfectionist::map_or_else` hand it
//! over as `MachineApplicable`.
//!
//! ## What this actually tests
//!
//! Not the suggestion text, which the `ui/` sweep pins, and not which
//! defaults qualify, which the same sweep pins case by case. What is
//! left is whether the text the rules splice parses and type-checks
//! where it lands, and whether the fixer leaves alone what the rules
//! decline.
//!
//! The shapes that earned the test are each a way the splice could go
//! wrong:
//!
//! - A mapper written as a path, where the rewrite has to carry the
//!   path over rather than a closure.
//! - A mapper spanning several lines, where the snippet taken from
//!   source has to land as written.
//! - A receiver that is a borrow, which the rewrite keeps, since it
//!   replaces the method and its arguments and nothing else.
//! - A `Result` whose default closure ignores the error, where
//!   `map_or_default` has to be handed the mapper alone.
//! - A constructor that is not its type's `Default`, which no rewrite
//!   may touch: the fixer reverts code that fails to compile, but
//!   `Port::new()` for `Port::default()` would compile and answer
//!   differently, so only comparing the whole file catches it.

pub mod _utils;

use _utils::{TempDir, build_project, cargo_manifest_dir, run_dylint_fix, shared_target_dir};
use std::fs;
use text_block_macros::text_block_fnl;

const SOURCE: &str = text_block_fnl! {
    r##"#![allow(dead_code, unused, reason = "fixture")]"##
    ""
    "pub struct Port(pub u16);"
    ""
    "impl Port {"
    "    pub fn new() -> Self {"
    "        Port(8080)"
    "    }"
    "}"
    ""
    "impl Default for Port {"
    "    fn default() -> Self {"
    "        Port(0)"
    "    }"
    "}"
    ""
    "pub fn literal_default(name: Option<&str>) -> usize {"
    "    name.map_or(0, str::len)"
    "}"
    ""
    "pub fn constructed_default(name: Option<&str>) -> String {"
    "    name.map_or(String::new(), |name| {"
    "        let trimmed = name.trim();"
    "        trimmed.to_uppercase()"
    "    })"
    "}"
    ""
    "pub fn through_a_reference(width: &Option<u32>) -> u32 {"
    "    width.map_or(Default::default(), |width| width + 1)"
    "}"
    ""
    "pub fn closure_default(name: Option<&str>) -> String {"
    "    name.map_or_else(String::new, str::to_uppercase)"
    "}"
    ""
    "pub fn error_ignored(name: Result<&str, u32>) -> String {"
    "    name.map_or_else(|_| String::new(), str::to_uppercase)"
    "}"
    ""
    "pub fn not_the_default(width: Option<u16>) -> Port {"
    "    width.map_or(Port::new(), Port)"
    "}"
};

const FIXED: &str = text_block_fnl! {
    r##"#![allow(dead_code, unused, reason = "fixture")]"##
    ""
    "pub struct Port(pub u16);"
    ""
    "impl Port {"
    "    pub fn new() -> Self {"
    "        Port(8080)"
    "    }"
    "}"
    ""
    "impl Default for Port {"
    "    fn default() -> Self {"
    "        Port(0)"
    "    }"
    "}"
    ""
    "pub fn literal_default(name: Option<&str>) -> usize {"
    "    name.map_or_default(str::len)"
    "}"
    ""
    "pub fn constructed_default(name: Option<&str>) -> String {"
    "    name.map_or_default(|name| {"
    "        let trimmed = name.trim();"
    "        trimmed.to_uppercase()"
    "    })"
    "}"
    ""
    "pub fn through_a_reference(width: &Option<u32>) -> u32 {"
    "    width.map_or_default(|width| width + 1)"
    "}"
    ""
    "pub fn closure_default(name: Option<&str>) -> String {"
    "    name.map_or_default(str::to_uppercase)"
    "}"
    ""
    "pub fn error_ignored(name: Result<&str, u32>) -> String {"
    "    name.map_or_default(str::to_uppercase)"
    "}"
    ""
    "pub fn not_the_default(width: Option<u16>) -> Port {"
    "    width.map_or(Port::new(), Port)"
    "}"
};

/// Run the fixer over the fixture and hand back what it left on disk,
/// plus its stderr.
fn fix() -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    build_project(
        temp.path(),
        "map_or_autofix",
        cargo_manifest_dir(),
        &[("src/lib.rs", SOURCE)],
    );
    let (stderr, success) = run_dylint_fix(temp.path(), &shared_target_dir());
    assert!(
        success,
        "`cargo dylint --fix` failed; stderr was:\n{stderr}",
    );
    let fixed = fs::read_to_string(temp.path().join("src/lib.rs")).expect("read fixed fixture");
    (temp, fixed, stderr)
}

#[test]
fn the_autofix_leaves_the_crate_compiling() {
    let (_temp, fixed, stderr) = fix();

    // The headline assertion. `cargo fix` prints this after applying a
    // suggestion that does not compile, having reverted the file -- so
    // its absence is what says every applied rewrite was sound.
    assert!(
        !stderr.contains("errors present after applying fixes"),
        "the autofix produced code that does not compile; stderr was:\n{stderr}",
    );

    // A revert would also leave the file untouched, which the assertion
    // above cannot tell from a rule that stood down. Comparing the
    // whole file pins every rewrite, and pins that `Port::new()` kept
    // its call.
    assert_eq!(
        fixed, FIXED,
        "the fixer did not turn the fixture into its expected form",
    );
}
