// edition:2024
#![feature(register_tool, postfix_match)]
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

// Not flagged: the one `&&` has a `let` beside it, so it is not
// counted. It is what makes the chain a chain; no binding replaces it.
fn let_chain(input: Option<u8>, ready: bool) {
    if let Some(_value) = input && ready {
        work();
    }
}

// Not flagged: every `&&` has a `let` beside it, and the trailing
// guard is one clause on its own rather than a part to name.
fn let_chain_all_lets(
    first: Option<u8>,
    second: Result<u8, ()>,
    third: Option<u8>,
    ready: bool,
) {
    if let Some(_one) = first
        && let Ok(_two) = second
        && let Some(_three) = third
        && ready
    {
        work();
    }
}

// Bad: 1 operator — the `&&` beside the `let` is not counted, the one
// joining the two ordinary clauses after it is.
fn let_chain_trailing_group(input: Option<u8>, first: bool, second: bool) {
    if let Some(_value) = input && first && second {
        work();
    }
}

// Bad: 1 operator — the same, with the part leading the chain.
fn let_chain_leading_group(input: Option<u8>, first: bool, second: bool) {
    if first && second && let Some(_value) = input {
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
// names that nothing else here reaches. The `&&` beside the `let` is
// not counted, so the head needs two ordinary clauses to be flagged.
fn while_let(mut items: impl Iterator<Item = u8>, ready: bool, more: bool) {
    while let Some(_item) = items.next()
        && ready
        && more
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

// Bad: 2 operators — the `&&` in the head, and the one in the nested
// `match`'s scrutinee. The arms are code the condition selects
// between; the scrutinee is a test it evaluates, and nothing else
// reaches it.
fn nested_match_scrutinee(first: bool, second: bool, third: bool) {
    if first && (match second && third {
        true => false,
        false => true,
    }) {
        work();
    }
}

macro_rules! both {
    ($first:expr, $second:expr) => {
        $first && $second
    };
}

// Bad: 1 operator, not 2 — the author wrote the head, so the condition
// is measured, but the `&&` inside the expansion is not the author's
// and does not count. Only the guard in the walk keeps it out; the one
// on the condition never sees it.
fn user_head_macro_inside(first: bool, second: bool, third: bool) {
    if first && both!(second, third) {
        work();
    }
}

// Bad: 1 operator — a postfix `match` is as author-written as any
// other, so its arm bodies are not part of the condition either.
fn nested_postfix_match(first: bool, second: bool, third: bool, value: u8) {
    if first
        && value.match {
            0 => second && third,
            _ => false,
        }
    {
        work();
    }
}

// Bad: 2 operators — the `&&` the author wrote in the head, and the
// one they wrote inside the macro's arguments. An argument keeps its
// call-site span, so it is the author's however the macro uses it; the
// expansion's own `&&` is still not counted.
fn macro_argument_operators(first: bool, second: bool, third: bool, fourth: bool) {
    if first && both!(second && third, fourth) {
        work();
    }
}

fn main() {}
