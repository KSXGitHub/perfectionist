// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// With `min_then_statements = 0` every `if` with a trivial `else` in a
// valid position is flagged however short its branch, which pins what counts
// as a trivial exit and as a valid position.

fn work(value: u32) -> u32 {
    value
}

// Flagged: `return`.
fn returns(ready: bool) {
    if ready {
        work(1);
    } else {
        return;
    }
}

// Flagged: `return` with a simple value.
fn returns_value(ready: bool) -> u32 {
    if ready {
        work(1);
    } else {
        return 0;
    }
    2
}

// Flagged: `break` and `continue`.
fn loops(ready: bool) {
    loop {
        if ready {
            work(1);
        } else {
            break;
        }
    }
    for _ in 0..2 {
        if ready {
            work(1);
        } else {
            continue;
        }
    }
}

// Flagged: bare values at the end of the function body — `None`,
// `false`, `Ok(())`, `()`.
fn value_none(ready: bool) -> Option<u32> {
    if ready { Some(work(1)) } else { None }
}

fn value_false(ready: bool) -> bool {
    if ready { work(1) > 0 } else { false }
}

fn value_ok(ready: bool) -> Result<(), u32> {
    if ready { Ok(work(1)).map(|_| ()) } else { Ok(()) }
}

// Not flagged: the `else` computes something.
fn else_computes(ready: bool) -> u32 {
    if ready { work(1) } else { work(2) }
}

// Not flagged: the `else` holds two statements.
fn else_two_statements(ready: bool) {
    if ready {
        work(1);
    } else {
        work(2);
        return;
    }
}

// Not flagged: an `else if` chain.
fn else_if(ready: bool, other: bool) {
    if ready {
        work(1);
    } else if other {
        work(2);
    } else {
        return;
    }
}

// Not flagged: the value of the `if` is used.
fn used(ready: bool) -> u32 {
    let value = if ready { work(1) } else { 0 };
    value
}

// Not flagged: a bare value at the tail of a block that is not the
// function body.
fn nested_value(ready: bool) -> u32 {
    let value = {
        if ready { work(1) } else { 0 }
    };
    value
}

// Flagged: a diverging `else` is fine at the tail of a nested block.
fn nested_diverging(ready: bool) -> u32 {
    let value = {
        if ready {
            work(1)
        } else {
            return 0;
        }
    };
    value
}

// Not flagged: the branch a `while` lowers to.
fn while_loop(mut count: u32) {
    while count > 0 {
        count -= 1;
    }
}

macro_rules! guarded {
    ($ready:expr) => {
        if $ready {
            work(1);
        } else {
            return;
        }
    };
}

// Not flagged: the `if` comes from a macro.
fn from_macro(ready: bool) {
    guarded!(ready);
}

fn main() {}
