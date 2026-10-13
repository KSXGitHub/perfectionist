// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// The rule ships inactive by default
// (`DEFAULT_STATE = DefaultState::Inactive` in
// `src/rules/multi_statement_iterator_closure.rs`), so without a
// configuration that enables it the pass never registers and nothing in
// this file produces a diagnostic. The callback below holds a `let` and
// a tail expression, which is 2 statements against the default
// `max_statements` of 1, so it would be flagged once the rule is on --
// which is what makes the empty `.stderr` mean something. The threshold
// and the shapes it counts are exercised under
// `ui-toml/multi_statement_iterator_closure/`, whose configurations
// `tests/multi_statement_iterator_closure.rs` supplies.

fn main() {
    let _ = [1, 2].into_iter().map(|value| {
        let doubled = value * 2;
        doubled
    });
}
