// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn work(value: u32) -> u32 {
    value
}

// Bad: every caller hard-codes `verbose`, and they disagree, so this is
// two functions sharing a body.
fn render(items: &[u32], verbose: bool) -> u32 {
    let total: u32 = items.iter().sum();
    if verbose { work(total) + items.len() as u32 } else { work(total) }
}

fn list(items: &[u32]) -> u32 {
    render(items, false)
}

fn list_verbose(items: &[u32]) -> u32 {
    render(items, true)
}

// Bad: every caller passes the same literal, so the parameter is a
// constant.
fn emit(value: u32, trailing_newline: bool) -> u32 {
    if trailing_newline { work(value) + 1 } else { work(value) }
}

fn emit_all(values: &[u32]) -> u32 {
    values.iter().map(|value| emit(*value, true)).sum()
}

// Bad: an `Option` parameter that is only ever `Some(..)` or `None`.
fn lookup(key: u32, fallback: Option<u32>) -> u32 {
    match fallback {
        Some(value) => work(key) + value,
        None => work(key),
    }
}

fn lookup_strict(key: u32) -> u32 {
    lookup(key, None)
}

fn lookup_lenient(key: u32) -> u32 {
    lookup(key, Some(0))
}

// Bad: a method's parameter is judged the same way.
struct Machine {
    state: u32,
}

impl Machine {
    fn step(&mut self, reset: bool) {
        if reset {
            self.state = 0;
        }
        self.state += 1;
    }

    fn run(&mut self) {
        self.step(true);
        self.step(false);
        Machine::step(self, false);
    }
}

// Good: a caller computes the value.
fn forwarded(items: &[u32], verbose: bool) -> u32 {
    if verbose { items.len() as u32 } else { 0 }
}

fn forward(items: &[u32], quiet: bool) -> u32 {
    forwarded(items, !quiet) + forwarded(items, true)
}

// Good: the function is also used as a value, so its callers cannot
// all be seen.
fn as_value(flag: bool) -> u32 {
    if flag { 1 } else { 0 }
}

fn uses_value() -> u32 {
    let function = as_value;
    as_value(true) + function(false)
}

// Good: a trait fixes the signature.
trait Toggle {
    fn toggle(&mut self, on: bool);
}

impl Toggle for Machine {
    fn toggle(&mut self, on: bool) {
        self.state = u32::from(on);
    }
}

fn toggles(machine: &mut Machine) {
    machine.toggle(true);
}

// Good: never called, so nothing can be said.
fn uncalled(flag: bool) -> u32 {
    if flag { 1 } else { 0 }
}

// Good: an argument that comes from a macro expansion is computed as far
// as the rule can tell.
macro_rules! call_with {
    ($flag:expr) => {
        forwarded(&[], $flag)
    };
}

fn from_macro() -> u32 {
    call_with!(true)
}

fn main() {}
