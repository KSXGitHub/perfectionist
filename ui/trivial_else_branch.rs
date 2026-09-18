// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn work(value: u32) -> u32 {
    value
}

// Bad: a three-statement branch guarded by an `else { return }`.
fn upside_down(ready: bool) {
    if ready {
        let first = work(1);
        let second = work(first);
        work(second);
    } else {
        return;
    }
}

// Good: the same as a guard clause.
fn guard_first(ready: bool) {
    if !ready {
        return;
    }
    let first = work(1);
    let second = work(first);
    work(second);
}

// Bad: an `if let` whose `else` continues the loop.
fn if_let_in_loop(items: &[Option<u32>]) {
    for item in items {
        if let Some(value) = item {
            let first = work(*value);
            let second = work(first);
            work(second);
        } else {
            continue;
        }
    }
}

// Good: `let ... else`.
fn let_else_in_loop(items: &[Option<u32>]) {
    for item in items {
        let Some(value) = item else {
            continue;
        };
        let first = work(*value);
        let second = work(first);
        work(second);
    }
}

// Bad: at the end of a function body, a bare `None` is a return value.
fn value_else(input: Option<u32>) -> Option<u32> {
    if let Some(value) = input {
        let first = work(value);
        let second = work(first);
        Some(work(second))
    } else {
        None
    }
}

// Good: a one-expression branch reads fine with its `else`.
fn short(input: Option<u32>) -> Option<u32> {
    if let Some(value) = input { Some(work(value)) } else { None }
}

// Good: the `if` is a value fed to a `let`, so no early exit applies.
fn used_as_value(ready: bool) -> u32 {
    let result = if ready {
        let first = work(1);
        let second = work(first);
        work(second)
    } else {
        0
    };
    result + 1
}

// Good: the `else` does work of its own.
fn busy_else(ready: bool) {
    if ready {
        let first = work(1);
        let second = work(first);
        work(second);
    } else {
        work(0);
        return;
    }
}

// Good: a bare value at the end of a nested block is not a return.
fn value_in_inner_block(ready: bool) -> u32 {
    let inner = {
        if ready {
            let first = work(1);
            let second = work(first);
            work(second)
        } else {
            0
        }
    };
    inner
}

fn main() {}
