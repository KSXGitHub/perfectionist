// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// The rule is inactive by default, so this directory's `dylint.toml`
// enables it and sets no knob; what it pins is the default limit of 5.

// Bad: 6 calls, one above the default limit of 5.
fn six_calls(names: &[String]) -> String {
    names
        .iter()
        .filter(|name| !name.is_empty())
        .map(|name| name.trim().to_owned())
        .rev()
        .collect::<Vec<_>>()
        .join(", ")
}

// Not flagged: a run of the same method is one call, so this builder
// has 2, `arg` and `status`.
fn builder() -> std::io::Result<std::process::ExitStatus> {
    std::process::Command::new("ls")
        .arg("-l")
        .arg("-a")
        .arg("-h")
        .arg("--color")
        .arg("/")
        .status()
}

// Good: the same pipeline as `six_calls` with its middle named.
fn named_stage(names: &[String]) -> String {
    let trimmed: Vec<String> = names
        .iter()
        .filter(|name| !name.is_empty())
        .map(|name| name.trim().to_owned())
        .collect();
    trimmed.join(", ")
}

// Not flagged: 5 calls is exactly the limit, and a chain is flagged
// only above the limit, never at it. No two adjacent calls share a
// method name, so nothing here collapses and the count is the number
// of calls written.
fn five_calls(names: &[String]) -> usize {
    names
        .iter()
        .filter(|name| !name.is_empty())
        .map(|name| name.trim().len())
        .take(3)
        .sum()
}

// Not flagged: a closure's chain is measured on its own, so 3 outside
// and 3 inside are two chains of 3.
fn chains_in_closures(rows: &[Vec<String>]) -> usize {
    rows.iter()
        .map(|row| row.iter().filter(|name| name.is_empty()).count())
        .sum()
}

struct Loader;

impl Loader {
    async fn load(&self) -> String {
        String::new()
    }
}

// Bad: 6 calls — an `.await` mid-chain neither counts nor breaks it,
// so `load` below the `.await` joins the 5 above it.
async fn through_await(loader: Loader) -> usize {
    loader
        .load()
        .await
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::len)
        .count()
}

// Bad: 6 calls — a `?` mid-chain is the same, so `trim` and `parse`
// below it join the 4 above.
fn through_try(input: &str) -> Result<usize, std::num::ParseIntError> {
    let count = input
        .trim()
        .parse::<u32>()?
        .to_string()
        .chars()
        .filter(char::is_ascii_digit)
        .count();
    Ok(count)
}

// Bad: 6 calls — `?` and `.await` do not break a chain.
async fn through_await_and_try(
    fetch: impl Future<Output = Result<String, ()>>,
) -> Result<usize, ()> {
    let count = fetch
        .await?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::len)
        .max()
        .unwrap_or(0);
    Ok(count)
}

// The 6 calls are above the limit, so this chain is what the macro
// below has to hold for the exemption to mean anything.
macro_rules! chained {
    ($items:expr) => {
        $items
            .iter()
            .copied()
            .map(|item| item + 1)
            .filter(|item| *item > 1)
            .rev()
            .sum::<u32>()
    };
}

// Not flagged: the calls come from the expansion.
fn built_from_a_macro(items: &[u32]) -> u32 {
    chained!(items)
}

macro_rules! source {
    ($items:expr) => {
        $items.iter().copied()
    };
}

// Not flagged: 5 calls. The head is written here, so the chain is
// measured, but the spine stops where the expansion starts rather than
// counting the 2 calls inside it.
fn head_over_a_macro_receiver(items: &[u32]) -> u32 {
    source!(items)
        .map(|item| item + 1)
        .filter(|item| *item > 1)
        .rev()
        .max()
        .unwrap_or(0)
}

fn main() {}
