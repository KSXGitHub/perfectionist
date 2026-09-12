// edition:2024
#![allow(dead_code, unused, reason = "ui fixture")]

// The rule ships inactive by default
// (`DEFAULT_STATE = DefaultState::Inactive` in
// `src/rules/overly_long_method_chain.rs`), so without a `dylint.toml`
// that enables it the pass never registers and nothing in this file
// produces a diagnostic. Both chains below are above the default limit
// of 5 and would be flagged once the rule is on, which is what makes
// the empty `.stderr` mean something. The limit and its knobs are
// exercised under `ui-toml/overly_long_method_chain/`, each with its
// own `dylint.toml` that opts the rule in.

fn six_calls(names: &[String]) -> String {
    names
        .iter()
        .filter(|name| !name.is_empty())
        .map(|name| name.trim().to_owned())
        .rev()
        .collect::<Vec<_>>()
        .join(", ")
}

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

fn main() {}
