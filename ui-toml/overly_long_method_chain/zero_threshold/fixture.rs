// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// With `max_calls = 0` every chain is flagged and the diagnostic states
// its count, which pins where a chain starts and ends.

struct Holder {
    items: Vec<u32>,
}

impl Holder {
    fn items(&self) -> &[u32] {
        &self.items
    }
}

// 1.
fn one_call(items: &[u32]) -> usize {
    items.len()
}

// 3.
fn three_calls(items: &[u32]) -> u32 {
    items.iter().copied().sum()
}

// 2: a field access starts the chain; it is not a call.
fn field_then_calls(holder: &Holder) -> usize {
    holder.items.iter().count()
}

// 2: a function call starts the chain; it is not a method call.
fn function_then_calls() -> usize {
    Vec::<u32>::new().iter().count()
}

// 2: `?` runs through the chain without counting.
fn through_try(input: Result<String, ()>) -> Result<usize, ()> {
    let count = input?.trim().len();
    Ok(count)
}

// 3 and 1: the closure's chain is its own.
fn closure_chain(rows: &[Vec<u32>]) -> usize {
    rows.iter().map(|row| row.len()).count()
}

// 2 and 1: an argument's chain is its own too.
fn argument_chain(items: &[u32], other: &[u32]) -> bool {
    items.iter().eq(other.iter())
}

// 2: a run of the same method is one step, so a builder is measured by
// its distinct steps, `arg` and `status`; `new` is a function call.
fn builder() -> std::io::Result<std::process::ExitStatus> {
    std::process::Command::new("ls")
        .arg("-l")
        .arg("-a")
        .arg("-h")
        .arg("/")
        .status()
}

// 1: a method call written as a macro argument counts; the expansion
// does not.
fn call_in_macro_argument(items: &[u32]) {
    println!("{}", items.len());
}

fn main() {}
