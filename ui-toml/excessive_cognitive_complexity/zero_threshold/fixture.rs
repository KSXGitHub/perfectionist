// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::future::Future;

fn work() {}

fn flag() -> bool {
    true
}

// Not flagged: 0.
fn straight_line(items: &[u8]) -> usize {
    let count = items.len();
    work();
    count
}

// Bad: a single `if`.
fn one_if(first: bool) {
    if first {
        work();
    }
}

// Bad: `if`, `else`.
fn if_else(first: bool) {
    if first {
        work();
    } else {
        work();
    }
}

// Bad: `if`, `else if`, `else` — the chain adds no nesting.
fn else_if_chain(n: u8) {
    if n == 0 {
        work();
    } else if n == 1 {
        work();
    } else {
        work();
    }
}

// Bad: `for` 1; the `if` pays 1 for nesting; `else if` and `else` each add 1
// with no nesting penalty, since they continue the `if` the reader is in.
fn nested_else_if(items: &[u8]) {
    for item in items {
        if *item == 0 {
            work();
        } else if *item == 1 {
            work();
        } else {
            work();
        }
    }
}

// Bad: the inner `if` pays 1 for nesting.
fn nested_if(first: bool, second: bool) {
    if first {
        if second {
            work();
        }
    }
}

// Bad: `for`, then an `if` nested inside it.
fn for_loop(items: &[u8]) {
    for item in items {
        if *item > 1 {
            work();
        }
    }
}

// Bad: the `if` a `while` lowers to is not a branch of its own.
fn while_loop(mut n: u8) {
    while n > 0 {
        n -= 1;
    }
}

// Bad: `while let` is a `while`.
fn while_let(mut items: impl Iterator<Item = u8>) {
    while let Some(_item) = items.next() {
        work();
    }
}

// Bad: the `loop`; an unlabelled `break` is free.
fn bare_loop() {
    loop {
        break;
    }
}

// Bad: `for` 1, `for` 2, `if` 3, labelled `continue` 1.
fn labelled_continue() {
    'outer: for first in 0..3 {
        for second in 0..3 {
            if first == second {
                continue 'outer;
            }
        }
    }
}

// Bad: `match` 1, guard 1; the arms themselves are free.
fn matching(n: u8) {
    match n {
        0 => work(),
        1 if flag() => work(),
        _ => {}
    }
}

// Bad: one `&&` run and one `||` run.
fn boolean_runs(first: bool, second: bool, third: bool, fourth: bool) -> bool {
    first && second && third || fourth
}

// Bad: parentheses start a new run.
fn boolean_parenthesised(first: bool, second: bool, third: bool) -> bool {
    first && (second || third)
}

// Bad: one `&&` run; `!` is free.
fn negation(first: bool, second: bool) -> bool {
    !(first && second)
}

// Not flagged: `?` is free.
fn question_mark(input: Result<u8, ()>) -> Result<u8, ()> {
    let value = input?;
    Ok(value)
}

// Bad: `let ... else`.
fn let_else(input: Option<u8>) -> u8 {
    let Some(value) = input else {
        return 0;
    };
    value
}

// Bad: the `else` block nests the `if` (1 + 1); the `let ... else` adds 1.
fn let_else_nested(input: Option<u8>, flag: bool) -> u8 {
    let Some(value) = input else {
        if flag {
            return 1;
        }
        return 0;
    };
    value
}

// Bad: the closure nests the `if` (2) and the `else` adds 1.
fn closure_nesting(items: &[u8]) -> Vec<u8> {
    items
        .iter()
        .map(|item| if *item > 0 { 1 } else { 0 })
        .collect()
}

// Bad: `if` 1, `else` 1, the recursive call 1.
fn recursive(n: u32) -> u32 {
    if n == 0 { 0 } else { recursive(n - 1) }
}

// Bad: a method calling itself is recursion too.
struct Counter;

impl Counter {
    fn count_down(&self, n: u32) {
        if n > 0 {
            self.count_down(n - 1);
        }
    }
}

// Bad: the `if` written as a macro argument counts; the expansion does not.
fn branch_in_macro_argument(first: bool) {
    println!("{}", if first { 1 } else { 0 });
}

macro_rules! local_branchy {
    ($flag:expr) => {
        if $flag {
            work();
        } else {
            work();
        }
    };
}

// Not flagged: the branches come from the expansion.
fn built_from_a_local_macro(first: bool) {
    local_branchy!(first);
}

// Bad: the `if`; `.await` is free.
async fn awaiting(ready: impl Future<Output = bool>) {
    if ready.await {
        work();
    }
}

// Bad: `if let` 1, the `&&` joining the chain 1.
fn if_let_chain(input: Option<u8>, first: bool) {
    if let Some(_value) = input
        && first
    {
        work();
    }
}

// Bad for inner; outer is Not flagged — a nested function is scored on its own.
fn outer() {
    fn inner(first: bool) {
        if first {
            work();
        }
    }
    inner(true);
}

fn main() {}
