// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// With `closure_weight = 1` a stage carrying a multi-line closure
// counts like any other, so this chain is four calls rather than six
// and the default `max_calls` leaves it alone.
fn multiline_closures(rows: &[Vec<u32>]) -> Vec<u32> {
    rows.iter()
        .filter(|row| {
            let total: u32 = row.iter().sum();
            total > 10
        })
        .map(|row| {
            let doubled = row.len() * 2;
            u32::try_from(doubled).unwrap_or_default()
        })
        .collect()
}

fn main() {}
