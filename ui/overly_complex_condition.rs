// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn work() {}

// Bad: 4 operators, one above the default limit of 3.
fn four_operators(first: bool, second: bool, third: bool, fourth: bool, fifth: bool) {
    if first && second && third && fourth && fifth {
        work();
    }
}

// Good: the same predicate with its first half named.
fn named_half(first: bool, second: bool, third: bool, fourth: bool, fifth: bool) {
    let leading = first && second && third;
    if leading && fourth && fifth {
        work();
    }
}

// Not flagged: 3 operators is exactly the limit, and a condition is
// flagged only above the limit, never at it.
fn three_operators(first: bool, second: bool, third: bool, fourth: bool) {
    if first && second && third && fourth {
        work();
    }
}

// Bad: 4 operators each — a `while` condition and a match guard are
// conditions too.
fn other_heads(first: bool, second: bool, third: bool, fourth: bool, fifth: bool, value: u8) {
    while first || second || third || fourth || fifth {
        work();
    }
    match value {
        0 if first && second && third && fourth && fifth => work(),
        _ => {}
    }
}

// Not flagged: 1 operator. A closure inside the condition is a scope
// of its own, so the 3 inside it belong to the closure.
fn closure_inside(items: &[bool], flag: bool) {
    if flag && items.iter().any(|item| *item && flag && !flag && flag) {
        work();
    }
}

// Not flagged: the operators come from a macro expansion.
macro_rules! all_of {
    ($first:expr, $second:expr, $third:expr, $fourth:expr, $fifth:expr) => {
        $first && $second && $third && $fourth && $fifth
    };
}

fn from_a_macro(first: bool, second: bool, third: bool, fourth: bool, fifth: bool) {
    if all_of!(first, second, third, fourth, fifth) {
        work();
    }
}

fn main() {}
