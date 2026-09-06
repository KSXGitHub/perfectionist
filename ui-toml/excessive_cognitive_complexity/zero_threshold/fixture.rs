// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// With `max_complexity = 0` every function whose score is at least 1 is
// flagged and the diagnostic states its score, which pins the increment
// each construct earns.

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

// 1.
fn one_if(first: bool) {
    if first {
        work();
    }
}

// 2: `if`, `else`.
fn if_else(first: bool) {
    if first {
        work();
    } else {
        work();
    }
}

// 3: `if`, `else if`, `else` — the chain adds no nesting.
fn else_if_chain(n: u8) {
    if n == 0 {
        work();
    } else if n == 1 {
        work();
    } else {
        work();
    }
}

// 3: the inner `if` pays 1 for nesting.
fn nested_if(first: bool, second: bool) {
    if first {
        if second {
            work();
        }
    }
}

// 3: `for`, then an `if` nested inside it.
fn for_loop(items: &[u8]) {
    for item in items {
        if *item > 1 {
            work();
        }
    }
}

// 1: the `if` a `while` lowers to is not a branch of its own.
fn while_loop(mut n: u8) {
    while n > 0 {
        n -= 1;
    }
}

// 1: `while let` is a `while`.
fn while_let(mut items: impl Iterator<Item = u8>) {
    while let Some(_item) = items.next() {
        work();
    }
}

// 1: an unlabelled `break` is free.
fn bare_loop() {
    loop {
        break;
    }
}

// 7: `for` 1, `for` 2, `if` 3, labelled `continue` 1.
fn labelled_continue() {
    'outer: for first in 0..3 {
        for second in 0..3 {
            if first == second {
                continue 'outer;
            }
        }
    }
}

// 2: `match` 1, guard 1; the arms themselves are free.
fn matching(n: u8) {
    match n {
        0 => work(),
        1 if flag() => work(),
        _ => {}
    }
}

// 2: one `&&` run and one `||` run.
fn boolean_runs(first: bool, second: bool, third: bool, fourth: bool) -> bool {
    first && second && third || fourth
}

// 2: parentheses start a new run.
fn boolean_parenthesised(first: bool, second: bool, third: bool) -> bool {
    first && (second || third)
}

// 1: `!` is free.
fn negation(first: bool, second: bool) -> bool {
    !(first && second)
}

// Not flagged: `?` is free.
fn question_mark(input: Result<u8, ()>) -> Result<u8, ()> {
    let value = input?;
    Ok(value)
}

// 1: `let ... else`.
fn let_else(input: Option<u8>) -> u8 {
    let Some(value) = input else {
        return 0;
    };
    value
}

// 3: the closure nests the `if` (2) and the `else` adds 1.
fn closure_nesting(items: &[u8]) -> Vec<u8> {
    items
        .iter()
        .map(|item| if *item > 0 { 1 } else { 0 })
        .collect()
}

// 3: `if` 1, `else` 1, the recursive call 1.
fn recursive(n: u32) -> u32 {
    if n == 0 { 0 } else { recursive(n - 1) }
}

// 2: a method calling itself is recursion too.
struct Counter;

impl Counter {
    fn count_down(&self, n: u32) {
        if n > 0 {
            self.count_down(n - 1);
        }
    }
}

// 2: the `if` written as a macro argument counts; the expansion does not.
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

// 1: `.await` is free.
async fn awaiting(ready: impl Future<Output = bool>) {
    if ready.await {
        work();
    }
}

// 2: `if let` 1, the `&&` joining the chain 1.
fn if_let_chain(input: Option<u8>, first: bool) {
    if let Some(_value) = input && first {
        work();
    }
}

// Outer not flagged, inner 1: a nested function is scored on its own.
fn outer() {
    fn inner(first: bool) {
        if first {
            work();
        }
    }
    inner(true);
}

fn main() {}
