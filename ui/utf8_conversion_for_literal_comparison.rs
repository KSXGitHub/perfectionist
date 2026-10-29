// edition:2024
//
// Which conversions this rule reads as meeting only string literals,
// which comparison each is asked to become, and which shapes it leaves
// alone.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::ffi::{CStr, CString, OsStr, OsString};
use std::path::{Path, PathBuf};
use std::str;

// Bad: an equality over an OS string, which can keep its literal.
fn equality_over_os_strings(value: &OsStr, owned: OsString) {
    if value.to_str() == Some("true") {}
    if value.to_str() != Some("true") {}
    // The literal may sit on either side.
    if Some("true") == value.to_str() {}
    if owned.into_string().unwrap() == "true" {}
}

// Bad: an equality over a path, which reaches its `OsStr` through
// `as_os_str`.
fn equality_over_paths(path: &Path, owned: PathBuf) {
    if path.to_str() == Some("main.rs") {}
    if owned.to_str() == Some("main.rs") {}
}

// Bad: an equality over a byte buffer, which is already its own native
// form.
fn equality_over_bytes(bytes: &[u8], owned: Vec<u8>, boxed: Box<[u8]>) {
    if str::from_utf8(bytes) == Ok("yes") {}
    if String::from_utf8_lossy(bytes) == "yes" {}
    if String::from_utf8(owned).unwrap() == "yes" {}
    if str::from_utf8(&boxed).unwrap() == "yes" {}
}

// Bad: an equality over a C string, whose byte view drops the
// terminator the literal never mentions.
fn equality_over_c_strings(value: &CStr, owned: CString) {
    if value.to_str() == Ok("yes") {}
    if owned.to_str().unwrap() == "yes" {}
}

// Bad: the motivating shape, where the arms are deciding between three
// words and every one of them carries a `Some`.
fn match_over_an_os_string(value: &OsStr) -> Result<bool, ()> {
    match value.to_str() {
        Some("true") => Ok(true),
        Some("false") | Some("") => Ok(false),
        _ => Err(()),
    }
}

// Bad: a `matches!` is a `match`, so the same reading applies.
fn matches_over_a_path(path: &Path) -> bool {
    matches!(path.to_str(), Some("lib.rs" | "main.rs" | "mod.rs"))
}

// Bad: the conversion reached through an `Option` adapter. The
// comparison already carries the `Some` the adapter answers with.
fn through_an_option_adapter(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("main.rs")
}

// Bad: the same adapter under a `matches!`.
fn through_an_option_adapter_in_a_match(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("lib.rs" | "main.rs"),
    )
}

// Good: the comparisons the rule asks for. It does not fire on what it
// suggests.
fn rewritten(value: &OsStr, path: &Path, bytes: &[u8], c_string: &CStr) -> bool {
    let _ = value == OsStr::new("true");
    let _ = path.as_os_str() == OsStr::new("main.rs");
    let _ = bytes == b"yes";
    let _ = c_string.to_bytes() == b"yes";
    let _ = matches!(value.as_encoded_bytes(), b"true" | b"false" | b"");
    let _ = path.file_name() == Some(OsStr::new("main.rs"));
    matches!(
        path.file_name().map(OsStr::as_encoded_bytes),
        Some(b"lib.rs" | b"main.rs"),
    )
}

// Not flagged: the arm binds the decoded text, so the conversion is
// what lets the arm use it.
fn an_arm_binds_the_text(path: &Path) -> Option<String> {
    match path.to_str() {
        Some("-") => None,
        Some(other) => Some(other.to_owned()),
        None => None,
    }
}

// Not flagged: a guard reads more than which literal the text is.
fn a_guard_reads_the_binding(path: &Path) -> bool {
    match path.to_str() {
        Some(name) if name.starts_with("main") => true,
        _ => false,
    }
}

// Not flagged: the non-UTF-8 case has an outcome of its own, which a
// byte comparison cannot keep apart from unlisted text.
fn the_non_utf8_case_is_distinguished(path: &Path) -> u8 {
    match path.to_str() {
        Some("-") => 0,
        Some(_) => 1,
        None => 2,
    }
}

// Not flagged: the comparand is not a literal, so there is nothing to
// compare the native form against.
fn the_comparand_is_not_a_literal(path: &Path, wanted: &str) -> bool {
    path.to_str() == Some(wanted)
}

// Not flagged: a lossy conversion against a literal holding U+FFFD. A
// lone `0xFF` renders as U+FFFD and compares equal, where its encoded
// bytes do not.
fn a_lossy_conversion_against_a_replacement_character(path: &Path) -> bool {
    path.to_string_lossy() == "�"
}

// Not flagged: the receiver is already a `str`, which has no non-UTF-8
// native form to compare instead.
fn the_receiver_is_already_text(text: &str) -> bool {
    text.to_lowercase() == "true"
}

// Not flagged: a `Vec` over something other than bytes is not a byte
// buffer.
fn not_a_byte_buffer(values: Vec<u16>) -> bool {
    values.len() == 1
}

// Not flagged: the closure converts something other than its own
// parameter, so the comparison is not asking about the adapter's
// receiver.
fn the_closure_converts_elsewhere(path: &Path, other: &OsStr) -> bool {
    path.file_name().and_then(|_| other.to_str()) == Some("main.rs")
}

fn main() {}
