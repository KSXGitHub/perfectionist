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

fn record(length: usize) {}

fn label(length: usize) -> &'static str {
    match length {
        0 => "empty",
        _ => "filled",
    }
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
    lines.for_each(|line| record(line.trim().len()));
}

// Bad: a predicate-returning adapter, where the chain ends in a `bool`.
fn predicate_chain(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(|line| line.trim().is_empty())
}

// Bad: the last step stays with the adapter, so only the steps a
// leading `map` would take have to lift. `label`'s result borrows, and
// an owned item could not hand that back out of a `map`.
fn last_step_borrows(headers: std::vec::IntoIter<String>) -> Vec<&'static str> {
    headers.map(|header| label(header.len())).collect()
}

// Bad, with a caveat the rule does not see: an adapter taking `&mut
// self` leaves the receiver positioned and usable, and a leading `map`
// would move it, so a rewrite here needs a reborrow.
fn receiver_used_after(mut lines: std::vec::IntoIter<&'static str>) -> (bool, usize) {
    let empty = lines.any(|line| line.trim().is_empty());
    (empty, lines.count())
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

// Not flagged: an owned item, whose borrow would not outlive the `map`
// the step would move into.
fn owned_item(headers: std::vec::IntoIter<String>) -> Vec<String> {
    headers
        .map(|header| header.trim().to_ascii_lowercase())
        .collect()
}

// Not flagged: `filter_map`, `find_map` and `map_while` meet the same
// condition, and their closure returns an `Option`, so what splits
// inside one lifts into an adapter matched to that discipline rather
// than into a `map`.
fn parsing(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| line.trim().parse::<usize>().ok())
        .collect()
}

fn first_parsed(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(|line| line.trim().parse::<usize>().ok())
}

fn parsed_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map_while(|line| line.trim().parse::<usize>().ok())
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
    lines.inspect(|line| record(line.trim().len())).collect()
}

// Not flagged: `Option::map` is a method of the same name whose item
// the rule does not speak about.
fn option_map(header: Option<&'static str>) -> Option<usize> {
    header.map(|text| text.trim().len())
}

trait Mapper {
    fn map<Output>(self, body: impl FnOnce(&'static str) -> Output) -> Output;
}

impl Mapper for &'static str {
    fn map<Output>(self, body: impl FnOnce(&'static str) -> Output) -> Output {
        body(self)
    }
}

// Not flagged: nor is a trait of one's own, whose method of that name
// says nothing about how the item arrives.
fn another_trait_map(header: &'static str) -> usize {
    header.map(|text| text.trim().len())
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

// Bad: the chain stops where a step's own closure names the
// accumulator, so the two steps before it still split and the body
// keeps the `unwrap_or_else`.
fn accumulator_in_a_nested_closure(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| line.trim().parse().unwrap_or_else(|_| total))
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

// Bad: a `let` and the block holding it run when the block does, so
// the chain's position is the block's.
fn fold_with_a_let(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        let length = line.trim().len();
        total + length
    })
}

// Bad: a scrutinee runs.
fn scrutinee_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| match line.trim().len() {
        0 => total,
        length => total + length,
    })
}

// Bad: an `if` condition runs too.
fn condition_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        if line.trim().is_empty() {
            total
        } else {
            total + 1
        }
    })
}

// Not flagged: a branch of an `if` does not run every time the closure
// does.
fn branching_fold(flag: bool, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        if flag {
            total + line.trim().len()
        } else {
            total
        }
    })
}

// Not flagged: a `let`'s `else` block runs only where the pattern does
// not match.
fn let_else_fold(fallback: Option<usize>, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| {
        let Some(extra) = fallback else {
            return total + line.trim().len();
        };
        total + extra
    })
}

// Not flagged: nor does an arm of a `match`.
fn arm_fold(flag: bool, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| match flag {
        true => total + line.trim().len(),
        false => total,
    })
}

// Not flagged: a `loop` runs its body any number of times including
// none, and every shape the rule does not recognise answers the same
// way.
fn looping_fold(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| loop {
        break total + line.trim().len();
    })
}

// Bad: `?` is a `match` on its operand, and an operand runs.
fn fold_through_try(
    mut lines: std::vec::IntoIter<&'static str>,
) -> Result<usize, std::num::ParseIntError> {
    lines.try_fold(0usize, |total, line| Ok(total + line.trim().parse::<usize>()?))
}

// Bad: the rest of the adapter table, one case each, so an entry
// naming the wrong parameter or the wrong shape would show up here.
fn flattening(lines: std::vec::IntoIter<&'static str>) -> String {
    lines.flat_map(|line| line.trim().chars()).collect()
}

fn trying_each(mut lines: std::vec::IntoIter<&'static str>) -> Option<()> {
    lines.try_for_each(|line| Some(record(line.trim().len())))
}

fn every(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.all(|line| line.trim().is_empty())
}

fn first_empty(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.position(|line| line.trim().is_empty())
}

fn last_empty(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.rposition(|line| line.trim().is_empty())
}

fn try_total(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.try_fold(0, |total, line| Some(total + line.trim().len()))
}

fn total_from_the_right(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.rfold(0, |total, line| total + line.trim().len())
}

fn try_total_from_the_right(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.try_rfold(0, |total, line| Some(total + line.trim().len()))
}

fn running_total(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .scan(0, |total, line| Some(*total + line.trim().len()))
        .collect()
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

// Not flagged: a unary closure whose body is not the chain. The scope
// here is a closure whose whole job is the chain.
fn body_is_not_the_chain(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(|line| line.trim().len() + 1).collect()
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

fn main() {}
