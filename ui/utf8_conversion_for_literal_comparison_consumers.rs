// edition:2024
//
// Which comparisons this rule reads as measuring a conversion's text
// against string literals, and which arms, operators and comparands
// leave it with nothing to measure.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::ffi::{CStr, OsStr};
use std::path::Path;

// Bad: a `match` deciding which of three words a value is, where every
// arm carries a `Some` that no arm asked for.
fn match_over_an_os_string(value: &OsStr) -> Result<bool, ()> {
    match value.to_str() {
        Some("true") => Ok(true),
        Some("false") | Some("") => Ok(false),
        _ => Err(()),
    }
}

// Good: `match_over_an_os_string` as the rule asks for it, with the
// arms spelled as the words they decide between.
fn match_over_an_os_string_rewritten(value: &OsStr) -> Result<bool, ()> {
    match value.as_encoded_bytes() {
        b"true" => Ok(true),
        b"false" | b"" => Ok(false),
        _ => Err(()),
    }
}

// Bad: a `matches!` expands to a `match`, so the rule reads its arms
// as literal patterns.
fn matches_over_a_path(path: &Path) -> bool {
    matches!(path.to_str(), Some("lib.rs" | "main.rs" | "mod.rs"))
}

// Good: `matches_over_a_path` as the rule asks for it. A string
// literal in pattern position matches a `str` and nothing else, so
// this one compares bytes.
fn matches_over_a_path_rewritten(path: &Path) -> bool {
    matches!(
        path.as_os_str().as_encoded_bytes(),
        b"lib.rs" | b"main.rs" | b"mod.rs",
    )
}

// Bad: a `match` whose other arm names the conversion's own failure
// alongside the text it could not place.
fn the_other_arm_names_the_failure(value: &OsStr) -> u8 {
    match value.to_str() {
        Some("true") => 1,
        Some(_) | None => 0,
    }
}

// Good: `the_other_arm_names_the_failure` as the rule asks for it. The
// comparison left behind cannot fail, so the arm that named the
// failure becomes a wildcard.
fn the_other_arm_names_the_failure_rewritten(value: &OsStr) -> u8 {
    match value.as_encoded_bytes() {
        b"true" => 1,
        _ => 0,
    }
}

// Bad: a fallible conversion whose other arm names the error it
// reports.
fn the_other_arm_names_an_error(value: &CStr) -> u8 {
    match value.to_str() {
        Ok("yes") => 1,
        Ok(_) | Err(_) => 0,
    }
}

// Good: `the_other_arm_names_an_error` as the rule asks for it, the
// error arm becoming a wildcard for want of anything left to name.
fn the_other_arm_names_an_error_rewritten(value: &CStr) -> u8 {
    match value.to_bytes() {
        b"yes" => 1,
        _ => 0,
    }
}

// Bad: a guard on a literal arm. The guard weighs something other than
// the text, and a pattern naming only literals leaves it nothing of the
// text to read, so the rewrite carries the guard across untouched.
fn an_arm_carries_a_guard(value: &OsStr, strict: bool) -> u8 {
    match value.to_str() {
        Some("true") if strict => 1,
        _ => 0,
    }
}

// Good: `an_arm_carries_a_guard` as the rule asks for it, the guard
// standing where it was written.
fn an_arm_carries_a_guard_rewritten(value: &OsStr, strict: bool) -> u8 {
    match value.as_encoded_bytes() {
        b"true" if strict => 1,
        _ => 0,
    }
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

// Not flagged: the arm binds the decoded text for its guard to read, so
// the conversion is what lets the guard ask what it asks.
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

// Not flagged: the error arm binds what the conversion reported, so the
// `match` uses the conversion's answer rather than discarding it.
fn an_error_arm_binds_the_error(value: &CStr) -> usize {
    match value.to_str() {
        Ok("yes") => 0,
        Ok(_) => 1,
        Err(error) => error.valid_up_to(),
    }
}

// Not flagged: an alternation naming both a literal and everything
// else. The arm reaches every input, so the rewrite has nothing to put
// in place of the literal inside it.
fn an_alternation_mixes_a_literal_and_a_wildcard(path: &Path) -> u8 {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("lib.rs") => 1,
        Some("main.rs" | _) | None => 2,
    }
}

// Not flagged: no arm names a literal, so the `match` is not asking
// which word the text is.
fn no_arm_names_a_literal(value: &OsStr) -> u8 {
    match value.to_str() {
        _ => 1,
    }
}

const WANTED: &str = "main.rs";

// Not flagged: the arm names a constant rather than a literal. The
// rewrite spells its literals as bytes, which a constant's name does
// not carry.
fn an_arm_names_a_constant(path: &Path) -> u8 {
    match path.to_str() {
        Some(WANTED) => 1,
        _ => 0,
    }
}

// Not flagged: an ordering comparison rather than an equality. An
// `Option` orders its `None` below every `Some`, which no comparison
// of the value's own form reproduces.
fn an_ordering_comparison(value: &OsStr) -> bool {
    value.to_str() < Some("true")
}

// Not flagged: the comparand is not a literal, so there is nothing to
// compare the native form against.
fn the_comparand_is_not_a_literal(path: &Path, wanted: &str) -> bool {
    path.to_str() == Some(wanted)
}

struct Decoder {
    convert: fn(&OsStr) -> Option<&str>,
}

// Not flagged: the comparand is built by a call the rule cannot look
// inside, so there is no literal behind it.
fn the_comparand_is_a_supplied_call(decoder: &Decoder, value: &OsStr) -> bool {
    value.to_str() == (decoder.convert)(value)
}

macro_rules! word {
    () => {
        "true"
    };
}

// Not flagged: the literal itself comes from a macro, so a rewrite
// would name whatever that expands to today.
fn the_literal_comes_from_a_macro(value: &OsStr) -> bool {
    value.to_str() == Some(word!())
}

// Not flagged: an arm's literal comes from a macro. The rewrite
// spells out each literal it reads, so it would freeze that macro's
// expansion into the source.
fn an_arm_literal_comes_from_a_macro(value: &OsStr) -> u8 {
    match value.to_str() {
        Some(word!()) => 1,
        _ => 0,
    }
}

macro_rules! wrapped {
    ($literal:literal) => {
        Some($literal)
    };
}

// Not flagged: the comparand's wrapper comes from a macro, so a
// rewrite would edit the macro's own text through this one call site.
fn the_comparand_comes_from_a_macro(value: &OsStr) -> bool {
    value.to_str() == wrapped!("true")
}

fn main() {}
