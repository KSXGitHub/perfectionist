// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn work() {}

// Bad: `for`, `if let`, `match`, `if` — four levels, one above the
// default limit.
fn four_levels(entries: &[Option<u8>], limit: u8) {
    for entry in entries {
        if let Some(size) = entry {
            match size {
                0 => {}
                size => {
                    if *size > limit {
                        work();
                    }
                }
            }
        }
    }
}

// Good: the same logic with a guard and a `match` guard is two levels.
fn two_levels(entries: &[Option<u8>], limit: u8) {
    for entry in entries {
        let Some(size) = entry else {
            continue;
        };
        match size {
            size if *size > limit => work(),
            _ => {}
        }
    }
}

// Good: exactly three is not above the limit.
fn three_levels(first: bool, second: bool, third: bool) {
    if first {
        if second {
            if third {
                work();
            }
        }
    }
}

// Good: an `else if` chain stays at one level however long it is.
fn else_if_chain(value: u8) {
    if value == 0 {
        work();
    } else if value == 1 {
        work();
    } else if value == 2 {
        work();
    } else if value == 3 {
        work();
    } else {
        work();
    }
}

// Good: a macro expansion adds no levels of its own.
macro_rules! deeply {
    ($flag:expr) => {
        if $flag {
            if $flag {
                if $flag {
                    if $flag {
                        work();
                    }
                }
            }
        }
    };
}

fn built_from_a_macro(flag: bool) {
    deeply!(flag);
}

// Bad: a closure is a level, so `for`, closure, `if`, `match` is four.
struct Machine;

impl Machine {
    fn step(&self, inputs: &[Vec<bool>]) {
        for row in inputs {
            row.iter().for_each(|input| {
                if *input {
                    match row.len() {
                        0 => work(),
                        _ => {}
                    }
                }
            });
        }
    }
}

fn main() {}
