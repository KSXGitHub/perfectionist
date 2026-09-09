// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Regression: a function *produced by a macro* is not counted, even when
// the names it binds are the caller's own identifiers — which carry
// user-source spans and so are not skipped by the per-binding
// `from_expansion` guard. The function's `def_span.from_expansion()`
// guard is what suppresses it. A local `macro_rules!` expansion is not an
// *external* macro, so `report_in_external_macro: false` does not cover
// this case; the guard is doing the work here on its own.
//
// The caller supplies sixteen distinct names — one above the default
// limit of 15 — so removing the guard makes `macro_generated` fire.

macro_rules! bind_all {
    ($($name:ident),+ $(,)?) => {
        fn macro_generated() -> u32 {
            $( let $name = 0u32; )+
            0 $( + $name )+
        }
    };
}

bind_all!(n0, n1, n2, n3, n4, n5, n6, n7, n8, n9, n10, n11, n12, n13, n14, n15);

fn main() {}
