// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Bad: six calls, one above the default limit.
fn six_calls(names: &[String]) -> String {
    names
        .iter()
        .filter(|name| !name.is_empty())
        .map(|name| name.trim().to_owned())
        .rev()
        .collect::<Vec<_>>()
        .join(", ")
}

// Good: a run of the same method is one step, so this builder has
// two: `arg` and `status`.
fn builder() -> std::io::Result<std::process::ExitStatus> {
    std::process::Command::new("ls")
        .arg("-l")
        .arg("-a")
        .arg("-h")
        .arg("--color")
        .arg("/")
        .status()
}

// Good: the same pipeline with its middle named.
fn named_stage(names: &[String]) -> String {
    let trimmed: Vec<String> = names
        .iter()
        .filter(|name| !name.is_empty())
        .map(|name| name.trim().to_owned())
        .collect();
    trimmed.join(", ")
}

// Good: exactly five is not above the limit.
fn five_calls(names: &[String]) -> usize {
    names
        .iter()
        .filter(|name| !name.is_empty())
        .map(|name| name.trim())
        .map(str::len)
        .sum()
}

// Good: a closure's chain is measured on its own, so three outside and
// three inside are two chains of three.
fn chains_in_closures(rows: &[Vec<String>]) -> usize {
    rows.iter()
        .map(|row| row.iter().filter(|name| name.is_empty()).count())
        .sum()
}

// Bad: `?` and `.await` do not break a chain; this one has six calls.
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

// Good: a macro's expansion is not measured.
macro_rules! chained {
    ($items:expr) => {
        $items
            .iter()
            .map(|item| item + 1)
            .map(|item| item + 1)
            .map(|item| item + 1)
            .map(|item| item + 1)
            .map(|item| item + 1)
            .sum::<u32>()
    };
}

fn built_from_a_macro(items: &[u32]) -> u32 {
    chained!(items)
}

fn main() {}
