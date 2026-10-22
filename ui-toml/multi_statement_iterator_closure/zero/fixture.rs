// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn main() {
    // Not flagged: an empty body counts nothing, even at a limit of 0.
    [1, 2].into_iter().for_each(|_| {});
    // Bad: one expression body is one statement.
    let _ = [1, 2].into_iter().map(|value| value + 1);
    // Bad: one statement carrying a semicolon.
    [1, 2].into_iter().for_each(|value| { dbg!(value); });
}
