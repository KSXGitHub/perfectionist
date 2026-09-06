// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]
#![allow(
    perfectionist::literal_only_parameter,
    reason = "the nested function is called once, with a literal, to be measured"
)]

// With `max_depth = 0` every function that nests at all is flagged and
// the diagnostic states its depth, which pins what counts as a level.

use std::future::Future;

fn work() {}

// Not flagged: nothing nests.
fn flat() {
    work();
    work();
}

// 1.
fn one_if(ready: bool) {
    if ready {
        work();
    }
}

// 2: an `if` inside an `if`.
fn nested_if(first: bool, second: bool) {
    if first {
        if second {
            work();
        }
    }
}

// 1: `else if` stays at the outer `if`'s level, and the `else` body is
// inside that level too.
fn else_if_chain(value: u8) {
    if value == 0 {
        work();
    } else if value == 1 {
        work();
    } else {
        work();
    }
}

// 2: an `if` inside an `else if` body.
fn if_inside_else_if(value: u8, ready: bool) {
    if value == 0 {
        work();
    } else if value == 1 {
        if ready {
            work();
        }
    }
}

// 1: a `match`; its arms are inside it, not levels of their own.
fn matching(value: u8) {
    match value {
        0 => work(),
        _ => {
            work();
        }
    }
}

// 2: an `if` inside an arm's block body.
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

// 2: `for` around an `if`.
fn for_loop(items: &[u8]) {
    for item in items {
        if *item > 1 {
            work();
        }
    }
}

// 1: the `if` a `while` lowers to is not a level.
fn while_loop(mut count: u8) {
    while count > 0 {
        count -= 1;
    }
}

// 1: `while let` is a `while`.
fn while_let(mut items: impl Iterator<Item = u8>) {
    while let Some(_item) = items.next() {
        work();
    }
}

// 1: a bare `loop`.
fn bare_loop() {
    loop {
        break;
    }
}

// 2: a closure is a level, and the `if` inside it another.
fn closure(items: &[u8]) -> Vec<u8> {
    items
        .iter()
        .map(|item| if *item > 0 { 1 } else { 0 })
        .collect()
}

// 1: a free-standing block.
fn scoped_block() {
    {
        work();
    }
}

// 1: the block a `let` initialises from.
fn let_block() -> u8 {
    let value = {
        work();
        1
    };
    value
}

// 1: an `unsafe` block.
fn unsafe_block() -> u8 {
    let value = unsafe { core::mem::transmute::<u8, u8>(1) };
    value
}

// 1: the body of a `let ... else`.
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

// 1: an `async` body is not a level; the `if` inside it is one.
async fn awaiting(ready: impl Future<Output = bool>) {
    if ready.await {
        work();
    }
}

// 1: an `async` block is not a level either.
fn async_block(ready: bool) -> impl Future<Output = ()> {
    async move {
        if ready {
            work();
        }
    }
}

// 1: the `if` written as a macro argument counts; the expansion does
// not.
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

// Outer not flagged, inner 1: a nested function is measured on its own.
fn outer() {
    fn inner(ready: bool) {
        if ready {
            work();
        }
    }
    inner(true);
}

// 1: a method body is measured like a free function's.
struct Counter;

impl Counter {
    fn count(&self, ready: bool) {
        if ready {
            work();
        }
    }
}

fn main() {}
