#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::collections::HashMap;
use std::ffi::{CStr, CString, OsStr, OsString};
use std::path::{Path, PathBuf};

// Bad: `&Path` whose only use is `to_path_buf`.
fn push_path(list: &mut Vec<PathBuf>, item: &Path) {
    list.push(item.to_path_buf());
}

// Bad: `&str` whose only use is `to_owned`.
fn store(name: &str, registry: &mut HashMap<String, u32>) {
    registry.insert(name.to_owned(), 0);
}

// Bad: `&str` -> `String` via `to_string`.
fn shout(text: &str) -> String {
    text.to_string()
}

// Bad: `&[u8]` -> `Vec<u8>` via `to_vec`.
fn keep(bytes: &[u8]) -> Vec<u8> {
    bytes.to_vec()
}

// Bad: `&str` -> `String` via `String::from`.
fn from_str(name: &str) -> String {
    String::from(name)
}

// Bad: `&str` -> `String` via `into`.
fn into_string(name: &str) -> String {
    name.into()
}

// Bad: `&OsStr` -> `OsString`.
fn keep_os(value: &OsStr) -> OsString {
    value.to_os_string()
}

// Bad: `&CStr` -> `CString`.
fn keep_c(value: &CStr) -> CString {
    value.to_owned()
}

struct Builder;

impl Builder {
    // Bad: inherent method, `&str` only used to produce `String`.
    fn named(&self, value: &str) -> String {
        value.to_owned()
    }
}

// OK: the conversion is conditional (nested in an `if` arm).
fn conditional(flag: bool, name: &str) -> Option<String> {
    if flag {
        Some(name.to_owned())
    } else {
        None
    }
}

// OK: the parameter is also used in borrowed form (used more than once).
fn log_and_store(name: &str, registry: &mut HashMap<String, u32>) {
    eprintln!("inserting {name}");
    registry.insert(name.to_owned(), 0);
}

trait Sink {
    fn absorb(&self, value: &str);
}

struct Bucket;

impl Sink for Bucket {
    // OK: the signature is fixed by the trait, so the implementer
    // cannot change the parameter type.
    fn absorb(&self, value: &str) {
        let _owned = value.to_owned();
    }
}

// OK: `&mut` references are out of the conservative rule's scope.
fn via_mut(value: &mut str) -> String {
    value.to_owned()
}

// OK: an explicit named lifetime may tie the parameter to another
// parameter or the return type.
fn explicit<'a>(value: &'a str) -> String {
    value.to_owned()
}

struct Sanitized;

impl Sanitized {
    // An inherent `from` that does real work and returns `String`.
    fn from(value: &str) -> String {
        value.trim().to_owned()
    }
}

// OK: `Sanitized::from` is not `String`'s own `from`, so dropping the
// call in a suggestion would lose the trimming. The rule must not flag
// this even though the result type is `String`.
fn sanitize(value: &str) -> String {
    Sanitized::from(value)
}

fn main() {}
