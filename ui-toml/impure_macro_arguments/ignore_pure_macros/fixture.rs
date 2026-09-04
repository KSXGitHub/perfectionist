// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// `ignore_pure_macros` drops names from the curated pure-macro
// list. After putting `cfg` in `ignore_pure_macros`, the call is
// treated as a regular macro invocation again and the surrounding
// `debug_assert!` argument is flagged as impure.
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
