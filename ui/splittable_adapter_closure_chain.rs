// edition:2024
//
// What counts as a step, and how many. The chain is found by walking up
// from the item's one occurrence, so what ends it is a parent that applies
// something else: a call naming another of the closure's bindings, a second
// mention of the item, or an expansion the reader did not write.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.
//
// The items are `&str` wherever the rule is meant to fire, because a step
// borrowing an owned item cannot leave the closure that owns it, which
// `splittable_adapter_closure_liftability.rs` is about.

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
fn split_two_steps(headers: std::vec::IntoIter<&'static str>) -> Vec<String> {
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



// Good: one adapter per step. `String::len` is not a path, so the last
// step keeps a closure, which holds one step.
fn split_three_steps(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .map(|lowered| lowered.len())
        .collect()
}



// Bad: a call whose sole argument is the chain is a step too.
fn call_step(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(|line| parse(line.trim())).collect()
}



// Good: one adapter per step, the call among them.
fn split_call_step(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(str::trim).map(parse).collect()
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



// Not flagged: the same in a fold, where the chain is still what the
// closure always reaches.
fn named_twice_in_a_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| total + line.trim().len() + line.len())
}



// Not flagged: the item is named again inside a step's own closure,
// which a visitor stopping at the closure boundary would not see.
fn named_in_a_nested_closure(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| line.trim().parse::<usize>().unwrap_or_else(|_| line.len()))
        .collect()
}



// Bad: the chain stops where a step names the accumulator, which a
// leading `map` could not reach, and the two steps before it still
// split. The body keeps the `wrapping_mul`.
fn accumulator_in_a_step(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(1, |acc, line| line.trim().len().wrapping_mul(acc))
}



// Good: the two steps lifted, leaving the closure the one that reaches the
// accumulator.
fn split_accumulator_in_a_step(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines
        .map(str::trim)
        .map(str::len)
        .fold(1, |acc, length| length.wrapping_mul(acc))
}



// Bad: the chain stops where a step's own closure names the
// accumulator, so the two steps before it still split and the body
// keeps the `unwrap_or_else`.
fn accumulator_in_a_nested_closure(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| line.trim().parse().unwrap_or_else(|_| total))
}



// Good: the two steps lifted, leaving the closure the call whose own
// closure reaches the accumulator.
fn split_accumulator_in_a_nested_closure(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines
        .map(str::trim)
        .map(str::parse::<usize>)
        .fold(0, |total, parsed| parsed.unwrap_or_else(|_| total))
}



// Not flagged: a step whose callee is another of the closure's
// parameters, which a leading `map` could not reach.
fn callee_is_the_state(
    measure: fn(&str) -> usize,
    lines: std::vec::IntoIter<&'static str>,
) -> Vec<usize> {
    lines
        .scan(measure, |measure, line| Some(measure(line.trim())))
        .collect()
}



// Not flagged: a step naming a local the body declares stays out of the
// chain, which leaves one step here, because the lifted `map` would name
// the local outside the closure, where it does not exist.
fn step_names_a_local(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        let cap = total.max(8);
        total + line.len().min(cap)
    })
}



// Not flagged: a destructured parameter bottoms the chain out at a
// binding the pattern introduced, which is a different rewrite.
fn destructured(pairs: std::vec::IntoIter<(usize, &'static str)>) -> usize {
    pairs.fold(0, |total, (_key, value)| total + value.trim().len())
}



// Not flagged: the same, with no step that borrows.
fn destructured_without_a_borrow(pairs: std::vec::IntoIter<(usize, usize)>) -> usize {
    pairs.fold(0, |total, (_key, value)| total + value.wrapping_add(1).wrapping_mul(2))
}



macro_rules! trimmed_lengths {
    ($lines:expr) => {
        $lines.map(|line| line.trim().len()).collect::<Vec<_>>()
    };
}



// Not flagged: the adapter comes from the expansion.
fn built_from_a_macro(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    trimmed_lengths!(lines)
}



fn wrap(count: usize) -> usize {
    count + 1
}



macro_rules! wrapped {
    ($value:expr) => {
        wrap($value)
    };
}



// Not flagged: a step the reader did not write. A `macro_rules!` body
// holding a call around the item reads as a step, and a split whose
// boundary falls inside an expansion is not a rewrite they can make.
fn step_from_a_macro(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(|line| wrapped!(line.len())).collect()
}

fn main() {}
