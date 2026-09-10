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

// 1.
fn one_and(first: bool, second: bool) {
    if first && second {
        work();
    }
}

// 2: `&&` and `||` both count.
fn mixed(first: bool, second: bool, third: bool) {
    if first && second || third {
        work();
    }
}

// 1: the `&&` of a `let` chain counts.
fn let_chain(input: Option<u8>, ready: bool) {
    if let Some(_value) = input && ready {
        work();
    }
}

// 1: a `while` condition.
fn while_loop(mut count: u8, ready: bool) {
    while count > 0 && ready {
        count -= 1;
    }
}

// 1: a match guard.
fn guard(value: u8, first: bool, second: bool) {
    match value {
        0 if first || second => work(),
        _ => {}
    }
}

// 1: an operator inside a closure belongs to the closure; the one
// outside it counts.
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

// 2: `else if` conditions are conditions of their own.
fn else_if(first: bool, second: bool, third: bool) {
    if first {
        work();
    } else if second && third || first {
        work();
    }
}

fn main() {}
