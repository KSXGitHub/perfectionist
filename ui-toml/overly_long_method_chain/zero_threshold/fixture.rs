// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// The rule is inactive by default, so this directory's `dylint.toml`
// enables it and sets `max_calls = 0`. Every chain is then flagged and
// the diagnostic states its count, which pins where a chain starts and
// ends.

struct Holder {
    items: Vec<u32>,
}

impl Holder {
    fn items(&self) -> &[u32] {
        &self.items
    }
}

// Bad: 1 call.
fn one_call(items: &[u32]) -> usize {
    items.len()
}

// Bad: 3 calls.
fn three_calls(items: &[u32]) -> u32 {
    items.iter().copied().sum()
}

// Bad: 2 calls, a field access starting the chain without being one.
fn field_then_calls(holder: &Holder) -> usize {
    holder.items.iter().count()
}

// Bad: 2 calls, a function call starting the chain without being a
// method call.
fn function_then_calls() -> usize {
    Vec::<u32>::new().iter().count()
}

// Bad: 2 calls, `?` running through the chain without counting.
fn through_try(input: Result<String, ()>) -> Result<usize, ()> {
    let count = input?.trim().len();
    Ok(count)
}

// Bad: 3 calls and 1 call, the closure's chain being its own.
fn closure_chain(rows: &[Vec<u32>]) -> usize {
    rows.iter().map(|row| row.len()).count()
}

// Bad: 2 calls and 1 call, an argument's chain being its own.
fn argument_chain(items: &[u32], other: &[u32]) -> bool {
    items.iter().eq(other.iter())
}

// Bad: 2 calls. A run of the same method is one call, so a builder is
// measured by its distinct calls, `arg` and `status`; `new` is a
// function call.
fn builder() -> std::io::Result<std::process::ExitStatus> {
    std::process::Command::new("ls")
        .arg("-l")
        .arg("-a")
        .arg("-h")
        .arg("/")
        .status()
}

// Bad: 1 call, since a method call written as a macro argument counts
// and the expansion does not.
fn call_in_macro_argument(items: &[u32]) {
    println!("{}", items.len());
}

fn main() {}
