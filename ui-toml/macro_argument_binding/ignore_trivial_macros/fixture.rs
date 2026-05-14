// `ignore_trivial_macros` drops names from the curated trivial-macro
// list. After putting `cfg` in `ignore_trivial_macros`, the call is
// treated as a regular macro invocation again and the surrounding
// `debug_assert!` argument is flagged as non-trivial.
//
// `cfg!` is genuinely pure across the standard library, so this is a
// hypothetical example — its purpose is to exercise the override.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

fn check() {
    debug_assert!(cfg!(any()));
}

fn main() {
    check();
}
