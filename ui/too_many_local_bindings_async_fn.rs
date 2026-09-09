// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Regression: lowering an `async fn` re-binds each parameter inside the
// coroutine body as `let <param> = <param>;`, reusing the parameter's own
// author-written pattern and span. Those re-bindings are not local
// bindings the author wrote, so an `async fn` must be counted exactly
// like the sync function with the same body.

// Good: sixteen parameters, empty body — parameters are not counted.
async fn params_only(a1: u32, a2: u32, a3: u32, a4: u32, a5: u32, a6: u32, a7: u32, a8: u32, a9: u32, a10: u32, a11: u32, a12: u32, a13: u32, a14: u32, a15: u32, a16: u32, ) {}

// Bad: the parameter is still not counted, but the body binds sixteen.
async fn with_bindings(seed: u32) -> u32 {
    let one = seed + 1;
    let two = one + 1;
    let three = two + 1;
    let four = three + 1;
    let five = four + 1;
    let six = five + 1;
    let seven = six + 1;
    let eight = seven + 1;
    let nine = eight + 1;
    let ten = nine + 1;
    let eleven = ten + 1;
    let twelve = eleven + 1;
    let thirteen = twelve + 1;
    let fourteen = thirteen + 1;
    let fifteen = fourteen + 1;
    let sixteen = fifteen + 1;
    sixteen
}

fn main() {}
