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

enum EnumName {
    VariantName(u8),
}

// Not flagged: every `&&` here has a `let` beside it, so none is
// counted. A chain of bindings has no group of ordinary clauses to
// lift out, and the trailing guard is a single clause.
fn all_lets(
    first: Option<u8>,
    second: Result<u8, ()>,
    third: EnumName,
    fourth: Option<u8>,
    fifth: Result<u8, ()>,
    ready: bool,
) {
    if let Some(_one) = first
        && let Ok(_two) = second
        && let EnumName::VariantName(_three) = third
        && let Some(_four) = fourth
        && let Err(_five) = fifth
        && ready
    {
        work();
    }
}

// Bad: 4 operators. The `&&` beside the `let` is not counted, and the
// five ordinary clauses after it are a group that a closure could name.
fn a_group_after_the_let(
    input: Option<u8>,
    first: bool,
    second: bool,
    third: bool,
    fourth: bool,
    fifth: bool,
) {
    if let Some(_value) = input && first && second && third && fourth && fifth {
        work();
    }
}

fn main() {}
