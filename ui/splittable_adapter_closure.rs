// edition:2024
//
// Which closures the rule splits, and which it leaves alone.
//
// Every Prefer form the rule's own docs ask for is here too. A form the
// rule suggests and then fires on again is a false positive that reading
// the Avoid cases alone would never find.
//
// The items are `&str` wherever the rule is meant to fire, because a
// step borrowing an owned item cannot leave the closure that owns it.
// `owned_item` below is that case.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn parse(text: &str) -> usize {
    text.len()
}

// Bad: two steps on the item.
fn two_steps(headers: std::vec::IntoIter<&'static str>) -> Vec<String> {
    headers
        .map(|header| header.trim().to_ascii_lowercase())
        .collect()
}

// Good: one adapter per step, which is what the rule asks for.
fn split_pair(headers: std::vec::IntoIter<&'static str>) -> Vec<String> {
    headers
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .collect()
}

// Bad: three steps, so the count in the message is not always two.
fn three_steps(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| line.trim().to_ascii_lowercase().len())
        .collect()
}

// Bad: a call whose sole argument is the chain is a step too.
fn call_step(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(|line| parse(line.trim())).collect()
}

// Bad: a consumer rather than an adapter, whose item still enters by
// value and never comes back out.
fn consumer(lines: std::vec::IntoIter<&'static str>) {
    lines.for_each(|line| drop(line.trim().len()));
}

// Bad: a predicate-returning adapter, where the chain ends in a `bool`.
fn predicate_chain(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(|line| line.trim().is_empty())
}

// Not flagged: one step is already one adapter doing one thing.
fn one_step(headers: std::vec::IntoIter<&'static str>) -> Vec<String> {
    headers.map(|header| header.to_ascii_lowercase()).collect()
}

// Not flagged: the item is named twice, so each step's own closure would
// leave the second mention with no binding.
fn named_twice(pairs: std::vec::IntoIter<&'static str>) -> Vec<String> {
    pairs.map(|pair| pair.trim().to_owned() + pair).collect()
}

// Not flagged: an owned item, whose borrow would not outlive the `map`
// the step would move into.
fn owned_item(headers: std::vec::IntoIter<String>) -> Vec<String> {
    headers
        .map(|header| header.trim().to_ascii_lowercase())
        .collect()
}

// Not flagged: `filter` hands the item back downstream, so a leading
// `map` would change what every later adapter sees.
fn filtering(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| line.trim().is_empty()).collect()
}

// Not flagged: `inspect` passes the item on unchanged, for the same
// reason.
fn inspecting(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .inspect(|line| println!("{}", line.trim().len()))
        .collect()
}

// Bad: a binary closure, whose accumulator stays in the body while the
// whole chain lifts.
fn binary_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| total + line.trim().len())
}

// Good: the same fold with its steps lifted out.
fn split_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines
        .map(str::trim)
        .map(str::len)
        .fold(0, |total, length| total + length)
}

// Bad: the chain stops where a step names the accumulator, which a
// leading `map` could not reach, and the two steps before it still
// split. The body keeps the `wrapping_mul`.
fn accumulator_in_a_step(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(1, |acc, line| line.trim().len().wrapping_mul(acc))
}

// Not flagged: a chain the closure does not always reach. Lifting it
// would run the parse for every item rather than for none, which
// compiles and answers differently.
fn conditional(flag: bool, lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| if flag { parse(line.trim()) } else { 0 })
        .collect()
}

// Not flagged: the right operand of `&&` is skipped where the left
// settles the answer.
fn short_circuit(lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.fold(false, |seen, line| seen && line.trim().is_empty())
}

// Not flagged: a destructured parameter bottoms the chain out at a
// binding the pattern introduced, which is a different rewrite.
fn destructured(pairs: std::vec::IntoIter<(usize, &'static str)>) -> usize {
    pairs.fold(0, |total, (_key, value)| total + value.trim().len())
}

// Not flagged: a unary closure whose body is not the chain. The scope
// here is a closure whose whole job is the chain.
fn body_is_not_the_chain(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(|line| line.trim().len() + 1).collect()
}

fn main() {}
