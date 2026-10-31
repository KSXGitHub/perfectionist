//! End-to-end proof that `utf8_conversion_for_literal_comparison`'s
//! rewrite compiles where the rule hands it over as
//! `MachineApplicable`.
//!
//! ## What this actually tests
//!
//! Not the suggestion text, which the `ui/` sweep pins. A `.stderr`
//! cannot show applicability at all, and applicability is what decides
//! whether `cargo dylint --fix` touches a suggestion, so the fixer is
//! the only thing that answers two questions: whether the spliced text
//! parses and type-checks where it lands, and whether the rewrite that
//! drops a panic is held back.
//!
//! The shapes that earned the test are each a way the rewrite could
//! compile as something other than the comparison it replaced, or fail
//! to compile at all:
//!
//! - An OS string met by a string literal, which needs the `Some` the
//!   comparison carried to go with the conversion.
//! - The literal on the left, so the two operands swap the roles the
//!   rewrite gives them.
//! - A path, which reaches a comparable form only through `as_os_str`.
//! - A `Vec<u8>`, which meets a byte-string literal as it stands, and a
//!   `Box<[u8]>`, which meets one only once sliced.
//! - A `match`, whose scrutinee and every literal arm change together.
//! - A `match` whose other arm names the conversion's own failure,
//!   which the rewrite has to replace with a wildcard because the
//!   comparison it leaves behind cannot fail.
//! - A closure parameter, carrying a second layer of reference that a
//!   comparison against a literal does not reach through.
//! - A guard on a literal arm, which the rewrite carries across
//!   untouched.
//! - An adapter over a byte slice, whose receiver already answers in
//!   the form the comparison wants, so the closure goes entirely.
//! - A dereferenced source, where the method the rewrite appends binds
//!   tighter than the `*` and needs brackets.
//! - An `Option` adapter, where the comparison keeps its `Some`, with
//!   the literal on each side in turn. The slice is named only for the
//!   left-hand order, the right-hand one reading the argument of `Some`
//!   after the other operand has named the type, so the fixer is what
//!   holds each order to the spelling it needs.
//! - An `unwrap` between the conversion and the comparison, whose
//!   rewrite answers `false` where the original panicked and so is
//!   offered as `MaybeIncorrect`. The fixer has to leave it alone.

pub mod _utils;

use _utils::{
    TempDir, build_project_with_config, cargo_manifest_dir, run_dylint_fix, shared_target_dir,
};
use std::fs;
use text_block_macros::text_block_fnl;

/// `needless_borrowed_parameters` would rewrite the borrowed signatures
/// below, and `core_instead_of_std` and the import rules would rewrite
/// the `use` statements, either of which would make "did the fixer
/// touch this line?" answer the wrong question.
const CONFIG: &str = text_block_fnl! {
    "[perfectionist]"
    r#"disable = ["core_instead_of_std", "import_granularity_mismatch", "import_grouping_mismatch", "needless_borrowed_parameters"]"#
};

const SOURCE: &str = text_block_fnl! {
    r##"#![allow(dead_code, unused, reason = "fixture")]"##
    ""
    "use std::ffi::CStr;"
    "use std::ffi::OsStr;"
    "use std::path::Path;"
    "use std::str;"
    ""
    "pub fn an_os_string(value: &OsStr) -> bool {"
    r#"    value.to_str() == Some("true")"#
    "}"
    ""
    "pub fn the_literal_on_the_left(value: &OsStr) -> bool {"
    r#"    Some("true") == value.to_str()"#
    "}"
    ""
    "pub fn a_path(path: &Path) -> bool {"
    r#"    path.to_str() == Some("main.rs")"#
    "}"
    ""
    "pub fn a_byte_slice(bytes: &[u8]) -> bool {"
    r#"    str::from_utf8(bytes) == Ok("yes")"#
    "}"
    ""
    "pub fn a_byte_buffer(owned: Vec<u8>) -> bool {"
    r#"    String::from_utf8_lossy(&owned) == "yes""#
    "}"
    ""
    "pub fn a_boxed_buffer(boxed: Box<[u8]>) -> bool {"
    r#"    String::from_utf8_lossy(&boxed) == "yes""#
    "}"
    ""
    "pub fn a_c_string(value: &CStr) -> bool {"
    r#"    value.to_str() == Ok("yes")"#
    "}"
    ""
    "pub fn a_match(value: &OsStr) -> u8 {"
    "    match value.to_str() {"
    r#"        Some("true") => 1,"#
    r#"        Some("false") | Some("") => 0,"#
    "        _ => 2,"
    "    }"
    "}"
    ""
    "pub fn an_option_adapter(path: &Path) -> bool {"
    r#"    path.file_name().and_then(|name| name.to_str()) == Some("main.rs")"#
    "}"
    ""
    "pub fn an_option_adapter_reversed(path: &Path) -> bool {"
    r#"    Some("main.rs") == path.file_name().and_then(|name| name.to_str())"#
    "}"
    ""
    "pub fn a_failure_arm(value: &OsStr) -> u8 {"
    "    match value.to_str() {"
    r#"        Some("true") => 1,"#
    "        Some(_) | None => 0,"
    "    }"
    "}"
    ""
    "pub fn a_second_reference(names: &[&OsStr]) -> bool {"
    r#"    names.iter().any(|name| name.to_str() == Some("main.rs"))"#
    "}"
    ""
    "pub fn a_guarded_arm(value: &OsStr, strict: bool) -> u8 {"
    "    match value.to_str() {"
    r#"        Some("true") if strict => 1,"#
    "        _ => 0,"
    "    }"
    "}"
    ""
    "pub fn an_adapter_over_bytes(chunk: Option<&[u8]>) -> bool {"
    r#"    matches!(chunk.map(|bytes| str::from_utf8(bytes)), Some(Ok("yes")))"#
    "}"
    ""
    "pub fn a_dereferenced_source(path: &&Path) -> bool {"
    r#"    (*path).to_str() == Some("main.rs")"#
    "}"
    ""
    "pub fn an_unwrap_is_left_alone(value: &CStr) -> bool {"
    r#"    value.to_str().unwrap() == "yes""#
    "}"
};

const FIXED: &str = text_block_fnl! {
    r##"#![allow(dead_code, unused, reason = "fixture")]"##
    ""
    "use std::ffi::CStr;"
    "use std::ffi::OsStr;"
    "use std::path::Path;"
    "use std::str;"
    ""
    "pub fn an_os_string(value: &OsStr) -> bool {"
    r#"    value == "true""#
    "}"
    ""
    "pub fn the_literal_on_the_left(value: &OsStr) -> bool {"
    r#"    "true" == value"#
    "}"
    ""
    "pub fn a_path(path: &Path) -> bool {"
    r#"    path.as_os_str() == "main.rs""#
    "}"
    ""
    "pub fn a_byte_slice(bytes: &[u8]) -> bool {"
    r#"    bytes == b"yes""#
    "}"
    ""
    "pub fn a_byte_buffer(owned: Vec<u8>) -> bool {"
    r#"    owned == b"yes""#
    "}"
    ""
    "pub fn a_boxed_buffer(boxed: Box<[u8]>) -> bool {"
    r#"    &boxed[..] == b"yes""#
    "}"
    ""
    "pub fn a_c_string(value: &CStr) -> bool {"
    r#"    value.to_bytes() == b"yes""#
    "}"
    ""
    "pub fn a_match(value: &OsStr) -> u8 {"
    "    match value.as_encoded_bytes() {"
    r#"        b"true" => 1,"#
    r#"        b"false" | b"" => 0,"#
    "        _ => 2,"
    "    }"
    "}"
    ""
    "pub fn an_option_adapter(path: &Path) -> bool {"
    r#"    path.file_name().map(|value| value.as_encoded_bytes()) == Some(b"main.rs")"#
    "}"
    ""
    "pub fn an_option_adapter_reversed(path: &Path) -> bool {"
    r#"    Some(b"main.rs".as_slice()) == path.file_name().map(|value| value.as_encoded_bytes())"#
    "}"
    ""
    "pub fn a_failure_arm(value: &OsStr) -> u8 {"
    "    match value.as_encoded_bytes() {"
    r#"        b"true" => 1,"#
    "        _ => 0,"
    "    }"
    "}"
    ""
    "pub fn a_second_reference(names: &[&OsStr]) -> bool {"
    r#"    names.iter().any(|name| *name == "main.rs")"#
    "}"
    ""
    "pub fn a_guarded_arm(value: &OsStr, strict: bool) -> u8 {"
    "    match value.as_encoded_bytes() {"
    r#"        b"true" if strict => 1,"#
    "        _ => 0,"
    "    }"
    "}"
    ""
    "pub fn an_adapter_over_bytes(chunk: Option<&[u8]>) -> bool {"
    r#"    matches!(chunk, Some(b"yes"))"#
    "}"
    ""
    "pub fn a_dereferenced_source(path: &&Path) -> bool {"
    r#"    (*path).as_os_str() == "main.rs""#
    "}"
    ""
    "pub fn an_unwrap_is_left_alone(value: &CStr) -> bool {"
    r#"    value.to_str().unwrap() == "yes""#
    "}"
};

/// Run the fixer over the fixture and hand back what it left on disk,
/// plus its stderr.
fn fix() -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    build_project_with_config(
        temp.path(),
        "utf8_conversion_for_literal_comparison_autofix",
        cargo_manifest_dir(),
        &[("src/lib.rs", SOURCE)],
        CONFIG,
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
    // suggestion that does not compile, having reverted the file, so
    // its absence is what says every applied rewrite was sound.
    assert!(
        !stderr.contains("errors present after applying fixes"),
        "the autofix produced code that does not compile; stderr was:\n{stderr}",
    );

    // A revert would also leave the file untouched, which the assertion
    // above cannot tell from a rule that stood down. Comparing the whole
    // file pins every rewrite, down to the brackets.
    assert_eq!(
        fixed, FIXED,
        "the fixer did not turn the fixture into its expected form",
    );
}
