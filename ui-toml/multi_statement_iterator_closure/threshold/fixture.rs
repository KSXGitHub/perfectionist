// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn main() {
    // Bad: three statements, above the raised limit of 2.
    let _ = [1, 2].into_iter().map(|value| {
        let doubled = value * 2;
        doubled
    });
    // Not flagged: two statements, at the raised limit.
    let _ = [1, 2].into_iter().map(|value| {
        let doubled = value * 2;
        let incremented = doubled + 1;
        incremented
    });
}
