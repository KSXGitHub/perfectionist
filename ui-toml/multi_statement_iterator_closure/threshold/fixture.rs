// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn main() {
    let _ = [1, 2].into_iter().map(|value| {
        let doubled = value * 2;
        doubled
    });
    let _ = [1, 2].into_iter().map(|value| {
        let doubled = value * 2;
        let incremented = doubled + 1;
        incremented
    });
}
