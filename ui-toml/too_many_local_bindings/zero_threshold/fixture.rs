// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// With `max_bindings = 0` every function that binds at least one name
// is flagged and the diagnostic states the count, which pins what each
// binding shape contributes.

fn work(value: u32) -> u32 {
    value
}

// Not flagged: parameters are not counted.
fn parameters_only(first: u32, second: u32) -> u32 {
    first + second
}

// 1.
fn one_let(seed: u32) -> u32 {
    let value = work(seed);
    value
}

// 1: shadowing rebinds the same name.
fn shadowed(seed: u32) -> u32 {
    let value = work(seed);
    let value = work(value);
    value
}

// 2: a destructuring pattern binds each of its names.
fn destructured(pair: (u32, u32)) -> u32 {
    let (left, right) = pair;
    left + right
}

// 2: the `for` pattern and the `let`; the loop's own iterator binding is
// the compiler's.
fn for_pattern(items: &[u32]) -> u32 {
    let mut total = 0;
    for item in items {
        total += item;
    }
    total
}

// 1: `if let`.
fn if_let(maybe: Option<u32>) -> u32 {
    if let Some(found) = maybe { found } else { 0 }
}

// 1: `while let`.
fn while_let(mut items: impl Iterator<Item = u32>) {
    while let Some(next) = items.next() {
        work(next);
    }
}

// 1: `let ... else`.
fn let_else(maybe: Option<u32>) -> u32 {
    let Some(required) = maybe else {
        return 0;
    };
    required
}

// 2: each arm's bindings.
fn match_arms(maybe: Result<u32, u32>) -> u32 {
    match maybe {
        Ok(good) => good,
        Err(bad) => bad,
    }
}

// 2: a closure's parameters.
fn closure_params(items: &[(u32, u32)]) -> u32 {
    items.iter().map(|(first, second)| first + second).sum()
}

// 2: a closure's parameter and its own `let`.
fn closure_body(items: &[u32]) -> u32 {
    items
        .iter()
        .map(|item| {
            let doubled = item * 2;
            doubled
        })
        .sum()
}

// Not flagged: names beginning with `_` are ignored.
fn underscored(seed: u32) {
    let _ignored = work(seed);
    let _ = work(seed);
}

// 1: the `?` desugaring's own bindings are not counted; the `let` is.
fn question_mark(input: Result<u32, ()>) -> Result<u32, ()> {
    let value = input?;
    Ok(value)
}

// 1: `format!`'s internal bindings are not counted; the `let` is.
fn formatting(seed: u32) -> String {
    let text = format!("{seed} and {seed}");
    text
}

macro_rules! bind_twice {
    ($name:ident, $value:expr) => {
        let $name = $value;
        let generated = $name;
    };
}

// 1: a name the author passed to a macro counts; one the macro made up
// does not.
fn local_macro(seed: u32) {
    bind_twice!(chosen, seed);
}

// Outer not flagged, inner 1: a nested function is counted on its own.
fn outer() {
    fn inner(seed: u32) -> u32 {
        let value = work(seed);
        value
    }
    inner(1);
}

// 1: a method body is counted like a free function's.
struct Counter;

impl Counter {
    fn count(&self, seed: u32) -> u32 {
        let value = work(seed);
        value
    }
}

fn main() {}
