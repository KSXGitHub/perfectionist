#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(dead_code, unused_variables, reason = "ui fixture")]
#![warn(perfectionist::single_letter_const_generic)]

// `allowed_idents = ["N"]` exempts the conventional `N` identifier
// from the lint; every other single-letter const generic still
// fires.

struct Lanes<const N: usize, const K: usize>([u8; N], [u8; K]);

fn main() {}
