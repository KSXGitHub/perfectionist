// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(dead_code, unused_variables, reason = "ui fixture")]
#![warn(perfectionist::single_letter_static_item)]

use std::sync::atomic::AtomicUsize;

// `allowed_idents = ["N"]` exempts the conventional `N` identifier
// from the lint; every other single-letter `static` still fires.

static N: AtomicUsize = AtomicUsize::new(0);
static K: u64 = 4;

fn main() {}
