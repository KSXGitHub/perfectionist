// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn work() {}

// Bad: one `match` with a loop and a nested branch inside — 16 with
// the default limit of 15.
fn over_the_limit(kind: u8, items: &[u8], flag: bool) -> u32 {
    let mut total = 0;
    match kind {
        // +1
        0 => {
            for item in items {
                // +2
                if *item > 1 && flag {
                    // +3, +1 for `&&`
                    total += 1;
                } else if *item == 0 {
                    // +1
                    total += 2;
                } else {
                    // +1
                    total += 3;
                }
            }
        }
        1 => {
            while total < 10 {
                // +2
                if flag {
                    // +3
                    total += 1;
                }
                total += 1;
            }
        }
        _ => {
            if flag {
                // +2
                total = 9;
            }
        }
    }
    total
}

// Good: the same work split by kind, each function flat and well under
// the limit.
fn under_the_limit(kind: u8, items: &[u8], flag: bool) -> u32 {
    match kind {
        0 => count_listed(items, flag),
        1 => count_up_to_ten(flag),
        _ => if flag { 9 } else { 0 },
    }
}

fn count_listed(items: &[u8], flag: bool) -> u32 {
    items.iter().map(|item| weight(*item, flag)).sum()
}

fn weight(item: u8, flag: bool) -> u32 {
    if item > 1 && flag {
        return 1;
    }
    if item == 0 { 2 } else { 3 }
}

fn count_up_to_ten(flag: bool) -> u32 {
    let mut total = 0;
    while total < 10 {
        total += if flag { 2 } else { 1 };
    }
    total
}

// Good: exactly 15 is not above the limit.
fn at_the_limit(first: bool, second: bool, third: bool, fourth: bool, fifth: bool) {
    if first {
        // +1
        if second {
            // +2
            if third {
                // +3
                if fourth {
                    // +4
                    if fifth {
                        // +5
                        work();
                    }
                }
            }
        }
    }
}

// Good: a macro expansion is opaque, so the branches its body expands to
// cost nothing.
macro_rules! branchy {
    ($flag:expr) => {
        if $flag {
            if $flag {
                if $flag {
                    if $flag {
                        if $flag {
                            if $flag {
                                work();
                            }
                        }
                    }
                }
            }
        }
    };
}

fn built_from_a_macro(flag: bool) {
    branchy!(flag);
    branchy!(flag);
    branchy!(flag);
}

// Bad: a method body is measured like a free function's — 16.
struct Machine;

impl Machine {
    fn step(&self, state: u8, inputs: &[bool]) -> u8 {
        let mut next = state;
        for input in inputs {
            // +1
            match next {
                // +2
                0 if *input && next < 9 => next = 1, // +1, +1 for `&&`
                1 => {
                    if *input {
                        // +3
                        next = 2;
                    } else {
                        // +1
                        next = 0;
                    }
                }
                2 => {
                    while next < 5 {
                        // +3
                        if *input {
                            // +4
                            next += 1;
                        }
                        next += 1;
                    }
                }
                _ => {}
            }
        }
        next
    }
}

fn main() {}
