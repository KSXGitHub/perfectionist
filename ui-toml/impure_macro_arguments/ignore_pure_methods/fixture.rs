// `ignore_pure_methods` drops names from the curated pure-method
// list. After putting `as_ref` in `ignore_pure_methods`, the call
// is treated as a regular method call again and the surrounding
// `debug_assert!` argument is flagged as impure.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

fn check(value: &Option<u32>) {
    debug_assert!(value.as_ref().is_some());
}

fn main() {
    check(&Some(0));
}
