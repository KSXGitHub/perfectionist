// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// With `max_depth = 0` every function that nests at all is flagged and
// the diagnostic states its depth, which pins what counts as a level.

use std::future::Future;

fn work() {}

// Not flagged: nothing nests.
fn flat() {
    work();
    work();
}

// Bad: 1 level — a single `if`.
fn one_if(ready: bool) {
    if ready {
        work();
    }
}

// Bad: 2 levels — an `if` inside an `if`.
fn nested_if(first: bool, second: bool) {
    if first {
        if second {
            work();
        }
    }
}

// Bad: 1 level — `else if` stays at the outer `if`'s level, and
// the `else` body is inside that level too.
fn else_if_chain(value: u8) {
    if value == 0 {
        work();
    } else if value == 1 {
        work();
    } else {
        work();
    }
}

// Bad: 2 levels — an `if` inside an `else if` body.
fn if_inside_else_if(value: u8, ready: bool) {
    if value == 0 {
        work();
    } else if value == 1 {
        if ready {
            work();
        }
    }
}

// Bad: 1 level — an `if let` is a level like any `if`.
fn if_let(input: Option<u8>) {
    if let Some(_value) = input {
        work();
    }
}

// Bad: 1 level — a let chain binds a pattern like a bare `if let`
// does, so the path names it `if let` and the suggestion is `let ...
// else`.
fn if_let_chain(input: Option<u8>, ready: bool) {
    if let Some(_value) = input
        && ready
    {
        work();
    }
}

// Bad: 2 levels — an `if` inside an `if` that has an `else` cannot
// become `&&`, so the suggestion falls back to the general advice.
fn if_in_if_with_else(first: bool, second: bool) {
    if first {
        if second {
            work();
        }
    } else {
        work();
    }
}

// Bad: 1 level — a `match`; its arms are inside it, not levels of
// their own.
fn matching(value: u8) {
    match value {
        0 => work(),
        _ => {
            work();
        }
    }
}

// Bad: 2 levels — an `if` inside an arm's block body.
fn if_in_arm(value: u8, ready: bool) {
    match value {
        0 => {
            if ready {
                work();
            }
        }
        _ => {}
    }
}

// Bad: 2 levels — an `if` in an arm guard sits under the `match` just
// as one in an arm body does, but a guard must evaluate to `bool`, so
// such an `if` always has an `else` and never draws the arm-guard
// suggestion.
fn if_in_arm_guard(value: u8, ready: bool) {
    match value {
        n if (if ready { n > 1 } else { false }) => work(),
        _ => {}
    }
}

// Bad: 2 levels — `for` around an `if`.
fn for_loop(items: &[u8]) {
    for item in items {
        if *item > 1 {
            work();
        }
    }
}

// Bad: 1 level — the `if` a `while` lowers to is not a level.
fn while_loop(mut count: u8) {
    while count > 0 {
        count -= 1;
    }
}

// Bad: 1 level — `while let` is a `while`.
fn while_let(mut items: impl Iterator<Item = u8>) {
    while let Some(_item) = items.next() {
        work();
    }
}

// Bad: 1 level — a bare `loop`.
fn bare_loop() {
    loop {
        break;
    }
}

// Bad: 2 levels — a closure is a level, and the `if` inside it
// another.
fn closure(items: &[u8]) -> Vec<u8> {
    items
        .iter()
        .map(|item| if *item > 0 { 1 } else { 0 })
        .collect()
}

// Bad: 1 level — a free-standing block.
fn scoped_block() {
    {
        work();
    }
}

// Bad: 1 level — the block a `let` initialises from.
fn let_block() -> u8 {
    let value = {
        work();
        1
    };
    value
}

// Bad: 1 level — an `unsafe` block.
fn unsafe_block() -> u8 {
    let value = unsafe { core::mem::transmute::<u8, u8>(1) };
    value
}

// Bad: 1 level — the body of a `let ... else`.
fn let_else(input: Option<u8>) -> u8 {
    let Some(value) = input else {
        return 0;
    };
    value
}

// Not flagged: `?` is not a level.
fn question_mark(input: Result<u8, ()>) -> Result<u8, ()> {
    let value = input?;
    Ok(value)
}

// Bad: 1 level — an `async` body is not a level; the `if` inside
// it is one.
async fn awaiting(ready: impl Future<Output = bool>) {
    if ready.await {
        work();
    }
}

// Bad: 2 levels — an `async` closure is one the author wrote, so
// it is a level like any other closure, and the `if` inside it is
// another.
fn async_closure(ready: bool) {
    let _make = async move || {
        if ready {
            work();
        }
    };
}

// Bad: 1 level — an `async` block is not a level either.
fn async_block(ready: bool) -> impl Future<Output = ()> {
    async move {
        if ready {
            work();
        }
    }
}

// Bad: 1 level — the `if` written as a macro argument counts; the
// expansion does not.
fn branch_in_macro_argument(ready: bool) {
    println!("{}", if ready { 1 } else { 0 });
}

macro_rules! local_nested {
    ($flag:expr) => {
        if $flag {
            if $flag {
                work();
            }
        }
    };
}

// Not flagged: the levels come from the expansion.
fn built_from_a_local_macro(ready: bool) {
    local_nested!(ready);
}

macro_rules! local_guard {
    ($input:expr) => {
        let Some(_value) = $input else {
            return;
        };
    };
}

// Not flagged: the `let ... else` comes from the expansion too, even
// though a `let` statement reaches the walk as a statement rather
// than as an expression.
fn let_else_from_a_local_macro(input: Option<u8>) {
    local_guard!(input);
}

// Not flagged: a nested function is measured on its own, so nothing
// inside `inner` counts toward `outer`.
fn outer() {
    // Bad: 1 level — measured on its own, not as part of `outer`.
    fn inner(ready: bool) {
        if ready {
            work();
        }
    }
    inner(true);
}

// Bad: 1 level — a method body is measured like a free function's.
struct Counter;

impl Counter {
    fn count(&self, ready: bool) {
        if ready {
            work();
        }
    }
}

fn main() {}
