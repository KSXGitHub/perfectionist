// edition:2024
//
// Which values this rule reads as holding something other than UTF-8,
// which of their own forms the comparison over each is asked to take,
// and which types it leaves alone.

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

// Good: `equality_over_os_strings` as the rule asks for it. An `OsStr`
// carries `PartialEq<str>`, so every literal keeps its text.
fn equality_over_os_strings_rewritten(value: &OsStr, owned: OsString) {
    if value == "true" {}
    if value != "true" {}
    if "true" == value {}
    if owned == "true" {}
}

// Bad: an equality over a path, which reaches its `OsStr` through
// `as_os_str`.
fn equality_over_paths(path: &Path, owned: PathBuf) {
    if path.to_str() == Some("main.rs") {}
    if owned.to_str() == Some("main.rs") {}
}

// Good: `equality_over_paths` as the rule asks for it. Comparing the
// `Path` itself is not the counterpart: `Path` compares by normalised
// components, so it would answer differently.
fn equality_over_paths_rewritten(path: &Path, owned: PathBuf) {
    if path.as_os_str() == "main.rs" {}
    if owned.as_os_str() == "main.rs" {}
}

// Bad: an equality over a byte buffer, which is already its own native
// form.
fn equality_over_bytes(bytes: &[u8], owned: Vec<u8>, boxed: Box<[u8]>) {
    if str::from_utf8(bytes) == Ok("yes") {}
    if String::from_utf8_lossy(bytes) == "yes" {}
    if String::from_utf8(owned).unwrap() == "yes" {}
    if str::from_utf8(&boxed).unwrap() == "yes" {}
}

// Good: `equality_over_bytes` as the rule asks for it. A `Vec<u8>`
// meets the byte-string literal as it stands, where a `Box<[u8]>` has
// to be sliced to meet one.
fn equality_over_bytes_rewritten(bytes: &[u8], owned: Vec<u8>, boxed: Box<[u8]>) {
    if bytes == b"yes" {}
    if owned == b"yes" {}
    if &boxed[..] == b"yes" {}
}

// Bad: an equality over a fixed-size buffer, which meets a
// byte-string literal only at its own length.
fn equality_over_a_fixed_buffer(buffer: [u8; 3]) -> bool {
    str::from_utf8(&buffer) == Ok("yes")
}

// Good: `equality_over_a_fixed_buffer` as the rule asks for it, sliced
// so that the literal's length no longer has to agree.
fn equality_over_a_fixed_buffer_rewritten(buffer: [u8; 3]) -> bool {
    &buffer[..] == b"yes"
}

// Bad: an equality over a C string, whose byte view drops the
// terminator the literal never mentions.
fn equality_over_c_strings(value: &CStr, owned: CString) {
    if value.to_str() == Ok("yes") {}
    if owned.to_str().unwrap() == "yes" {}
}

// Good: `equality_over_c_strings` as the rule asks for it. Neither
// view is a `_with_nul` one, which would retain a terminator no
// literal can carry.
fn equality_over_c_strings_rewritten(value: &CStr, owned: CString) {
    if value.to_bytes() == b"yes" {}
    if owned.as_bytes() == b"yes" {}
}

// Bad: a dereferenced source, whose brackets the rewrite has to keep,
// a method appended to `*path` binding tighter than the `*` does.
fn a_dereferenced_source(path: &&Path) -> bool {
    (*path).to_str() == Some("main.rs")
}

// Good: `a_dereferenced_source` as the rule asks for it, the brackets
// still holding `as_os_str` to the path rather than to its result.
fn a_dereferenced_source_rewritten(path: &&Path) -> bool {
    (*path).as_os_str() == "main.rs"
}

// Bad: a closure parameter, carrying a second layer of reference that
// the comparison does not reach through.
fn through_a_second_reference(names: &[&OsStr]) -> bool {
    names.iter().any(|name| name.to_str() == Some("main.rs"))
}

// Good: `through_a_second_reference` as the rule asks for it, with the
// dereference that leaves the one layer the comparison reaches
// through.
fn through_a_second_reference_rewritten(names: &[&OsStr]) -> bool {
    names.iter().any(|name| *name == "main.rs")
}

// Not flagged: the receiver is already a `str`, which has no non-UTF-8
// native form to compare instead.
fn the_receiver_is_already_text(text: &str) -> bool {
    text.to_lowercase() == "true"
}

trait DecodeText {
    fn to_str(&self) -> Option<&str>;
}

impl DecodeText for Vec<u16> {
    fn to_str(&self) -> Option<&str> {
        None
    }
}

impl DecodeText for [u16] {
    fn to_str(&self) -> Option<&str> {
        None
    }
}

impl DecodeText for u16 {
    fn to_str(&self) -> Option<&str> {
        None
    }
}

impl DecodeText for Box<str> {
    fn to_str(&self) -> Option<&str> {
        None
    }
}

// Not flagged: a `Vec` over something other than bytes is not a byte
// buffer, whatever it names its decoding method.
fn not_a_byte_buffer(values: Vec<u16>) -> bool {
    values.to_str() == Some("yes")
}

// Not flagged: a slice over something other than bytes. The rule
// reads the element type rather than trusting the shape.
fn not_a_byte_slice(values: &[u16]) -> bool {
    values.to_str() == Some("yes")
}

// Not flagged: a value whose type is no kind of buffer at all.
fn not_a_buffer(value: u16) -> bool {
    value.to_str() == Some("yes")
}

// Not flagged: a `Box` over text rather than over a slice of bytes.
fn not_a_boxed_buffer(value: Box<str>) -> bool {
    value.to_str() == Some("yes")
}

mod elsewhere {
    pub struct Path(pub String);

    impl Path {
        pub fn to_str(&self) -> Option<&str> {
            Some(&self.0)
        }
    }
}

// Not flagged: a `Path` defined outside the standard library, which
// shares the name and holds UTF-8 already.
fn a_path_defined_elsewhere(path: &elsewhere::Path) -> bool {
    path.to_str() == Some("main.rs")
}

fn main() {}
