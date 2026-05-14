// `extra_trivial_macros` adds project-specific macro names to the
// built-in trivial-macro set. A call to one of these macros — even
// nested inside a deny-listed macro — qualifies as a trivial atom
// and the surrounding expression is accepted.
//
// This fixture's `dylint.toml` (see `tests/macro_argument_binding.rs`)
// adds `literal_table`; the call below would otherwise be flagged
// because macro invocations outside the built-in trivial set are
// non-trivial by default.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]

macro_rules! literal_table {
    ($key:ident) => {
        "table-literal"
    };
}

fn check() {
    debug_assert_eq!(literal_table!(KEY), "table-literal");
}

fn main() {
    check();
}
