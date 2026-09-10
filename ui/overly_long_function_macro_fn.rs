// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Regression: a function *produced by a macro* is not measured. The
// body below is written out in the macro, so the source the counter
// would read spans fifty-two lines -- two above the default limit --
// and `macro_generated` fires the moment the
// `def_span.from_expansion()` guard in `measured_fn` is removed. A
// local `macro_rules!` expansion is not an *external* macro, so
// `report_in_external_macro: false` does not cover this case; the
// guard is doing the work on its own.

macro_rules! long_fn {
    () => {
        fn macro_generated() -> u32 {
            let n0 = 0u32;
            let n1 = 0u32;
            let n2 = 0u32;
            let n3 = 0u32;
            let n4 = 0u32;
            let n5 = 0u32;
            let n6 = 0u32;
            let n7 = 0u32;
            let n8 = 0u32;
            let n9 = 0u32;
            let n10 = 0u32;
            let n11 = 0u32;
            let n12 = 0u32;
            let n13 = 0u32;
            let n14 = 0u32;
            let n15 = 0u32;
            let n16 = 0u32;
            let n17 = 0u32;
            let n18 = 0u32;
            let n19 = 0u32;
            let n20 = 0u32;
            let n21 = 0u32;
            let n22 = 0u32;
            let n23 = 0u32;
            let n24 = 0u32;
            let n25 = 0u32;
            let n26 = 0u32;
            let n27 = 0u32;
            let n28 = 0u32;
            let n29 = 0u32;
            let n30 = 0u32;
            let n31 = 0u32;
            let n32 = 0u32;
            let n33 = 0u32;
            let n34 = 0u32;
            let n35 = 0u32;
            let n36 = 0u32;
            let n37 = 0u32;
            let n38 = 0u32;
            let n39 = 0u32;
            let n40 = 0u32;
            let n41 = 0u32;
            let n42 = 0u32;
            let n43 = 0u32;
            let n44 = 0u32;
            let n45 = 0u32;
            let n46 = 0u32;
            let n47 = 0u32;
            let n48 = 0u32;
            let n49 = 0u32;
            let n50 = 0u32;
            n50
        }
    };
}

long_fn!();

fn main() {}
