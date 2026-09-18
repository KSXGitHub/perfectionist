// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn main() {
    [1, 2].into_iter().for_each(|_| {});
    let _ = [1, 2].into_iter().map(|value| value + 1);
    [1, 2].into_iter().for_each(|value| { dbg!(value); });
}
