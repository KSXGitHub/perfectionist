#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![allow(dead_code, unused_variables, reason = "ui fixture")]
#![warn(perfectionist::single_letter_const_item)]

// `allowed_idents = ["N"]` exempts the conventional `N` identifier
// from the lint; every other single-letter `const` still fires.

const N: usize = 2;
const K: usize = 4;

fn main() {}
