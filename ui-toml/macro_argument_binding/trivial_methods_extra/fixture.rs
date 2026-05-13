// `trivial_methods_extra` adds project-specific method names to the
// built-in pure-getter set. A `.method()` invocation on a trivial
// base then qualifies as a trivial postfix and the surrounding
// expression is accepted.
//
// This fixture's `dylint.toml` (see `tests/macro_argument_binding.rs`)
// adds `cached_size`; the call below would otherwise be flagged
// because zero-arg method calls outside the built-in set are
// non-trivial by default.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

struct Buffer {
    data: Vec<u32>,
}

impl Buffer {
    fn cached_size(&self) -> usize {
        self.data.len()
    }
}

fn check(buffer: &Buffer, expected: usize) {
    debug_assert!(buffer.cached_size() == expected);
    debug_assert!(buffer.cached_size() <= expected);
}

fn main() {
    let buffer = Buffer { data: vec![] };
    check(&buffer, 0);
}
