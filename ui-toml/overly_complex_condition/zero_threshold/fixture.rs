// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// With `max_operators = 0` every condition with an operator is flagged
// and the diagnostic states the count, which pins what is counted.

fn work() {}

// Not flagged: no operator.
fn plain(ready: bool) {
    if ready {
        work();
    }
}

// Bad: 1 operator.
fn one_and(first: bool, second: bool) {
    if first && second {
        work();
    }
}

// Bad: 2 operators — `&&` and `||` both count.
fn mixed(first: bool, second: bool, third: bool) {
    if first && second || third {
        work();
    }
}

// Bad: 1 operator — the `&&` of a `let` chain counts.
fn let_chain(input: Option<u8>, ready: bool) {
    if let Some(_value) = input && ready {
        work();
    }
}

// Bad: 1 operator — a `while` condition.
fn while_loop(mut count: u8, ready: bool) {
    while count > 0 && ready {
        count -= 1;
    }
}

// Bad: 1 operator — a match guard.
fn guard(value: u8, first: bool, second: bool) {
    match value {
        0 if first || second => work(),
        _ => {}
    }
}

// Bad: 1 operator — an operator inside a closure belongs to the
// closure; the one outside it counts.
fn closure_inside(items: &[bool], flag: bool) {
    if flag && items.iter().any(|item| *item || flag) {
        work();
    }
}

// Not flagged: `!` is not a boolean operator here.
fn negation(ready: bool) {
    if !ready {
        work();
    }
}

// Not flagged: the operators are in a `let`, which is the named form.
fn named(first: bool, second: bool, third: bool) {
    let all = first && second && third;
    if all {
        work();
    }
}

// Bad: 2 operators — an `else if` condition is a condition of its own.
fn else_if(first: bool, second: bool, third: bool) {
    if first {
        work();
    } else if second && third || first {
        work();
    }
}

// Bad: 1 operator — a `while let` condition, the one head the rustdoc
// names that nothing else here reaches.
fn while_let(mut items: impl Iterator<Item = u8>, ready: bool) {
    while let Some(_item) = items.next()
        && ready
    {
        work();
    }
}

// Bad: 1 operator, not 2 — a nested `if`'s branches are code this
// condition selects between, not part of what it tests, and the nested
// head is counted as the condition it is rather than a second time
// here.
fn nested_if(first: bool, second: bool, third: bool, fourth: bool) {
    if first && (if second { third } else { fourth }) {
        work();
    }
}

// Bad: 1 operator, and a second diagnostic for the nested head's own
// `&&`. Each is counted once, where it belongs.
fn nested_head(first: bool, second: bool, third: bool, fourth: bool, fifth: bool) {
    if first && (if second && third { fourth } else { fifth }) {
        work();
    }
}

// Bad: 1 operator — a nested `match`'s arm bodies are not part of the
// condition either.
fn nested_match(first: bool, second: bool, third: bool, value: u8) {
    if first && (match value {
        0 => second && third,
        _ => false,
    }) {
        work();
    }
}

fn main() {}
