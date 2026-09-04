// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// `apply_to_outer_scopes = true`: crate-level `#![allow(...)]` and
// outer `#[allow(...)]` on a `mod` item become eligible.

#![feature(register_tool)]
#![register_tool(perfectionist)]

// Bad — an outer `#[allow]` on a `mod` item is now rewriteable.
#[allow(non_snake_case, reason = "module-wide")]
mod outer_module {
    pub fn Item() {}
}

fn main() {
    // Bad — an inner attribute (`#![...]`) is now rewriteable.
    #![allow(clippy::needless_return, reason = "inner attr")]

    outer_module::Item();
}
