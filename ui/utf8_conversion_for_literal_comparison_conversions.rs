// edition:2024
//
// What this rule reads around a conversion: the calls it looks through
// to reach one, the calls it declines to look through, and the limit a
// conversion that replaces ill-formed input rather than reporting it
// puts on the rewrite.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::ffi::OsStr;
use std::path::Path;

// Bad: an `expect` rather than an `unwrap` between the conversion and
// the comparison. Either hands back the text the conversion produced.
fn an_expect_between(path: &Path) -> bool {
    path.to_str().expect("not UTF-8") == "main.rs"
}

// Good: `an_expect_between` as the rule asks for it. The rewrite drops
// the `expect`, so input the conversion would have rejected answers
// `false` where it used to panic.
fn an_expect_between_rewritten(path: &Path) -> bool {
    path.as_os_str() == "main.rs"
}

// Bad: a lossy conversion, which the rule reads beside the ones that
// report ill-formed input instead of replacing it.
fn a_lossy_conversion(path: &Path) -> bool {
    path.to_string_lossy() == "main.rs"
}

// Good: `a_lossy_conversion` as the rule asks for it. The replacement
// the conversion would make has no pre-image among the inputs that
// render as this literal, so the two answer alike.
fn a_lossy_conversion_rewritten(path: &Path) -> bool {
    path.as_os_str() == "main.rs"
}

// Not flagged: a conversion consumed through an adapter other than
// `unwrap` or `expect`, which the rule does not read through.
fn the_conversion_is_defaulted(value: &OsStr) -> bool {
    value.to_str().unwrap_or("") == "true"
}

// Not flagged: a lossy conversion against a literal holding U+FFFD. A
// lone `0xFF` renders as U+FFFD and compares equal, where its encoded
// bytes do not.
fn a_lossy_conversion_against_a_replacement_character(path: &Path) -> bool {
    path.to_string_lossy() == "�"
}

// Not flagged: a lossy conversion taken as a function, where the
// literal holds U+FFFD. The replacement character has pre-images among
// the ill-formed inputs, which the byte comparison does not share.
fn a_lossy_function_against_a_replacement_character(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes) == "�"
}

macro_rules! decoded {
    ($value:expr) => {
        $value.to_str()
    };
}

// Not flagged: the conversion itself comes from a macro, whose body is
// where the text a rewrite would name may live.
fn the_conversion_comes_from_a_macro(value: &OsStr) -> bool {
    decoded!(value).unwrap() == "true"
}

fn main() {}
