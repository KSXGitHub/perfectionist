// `extra_pure_macros` adds project-specific macro names to the
// built-in pure-macro set. A call to one of these macros — even
// nested inside a denied macro — qualifies as a pure atom
// and the surrounding expression is accepted.
//
// This fixture's `dylint.toml` (see `tests/impure_macro_arguments.rs`)
// adds `literal_table`; the call below would otherwise be flagged
// because macro invocations outside the built-in pure set are
// impure by default.

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
