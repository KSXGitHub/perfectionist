// edition:2024
//
// Where the chain can sit in the closure, and what the rule reads as running
// before it. Lifting a step moves it ahead of everything the closure did
// first, so an expression with an effect that precedes the chain is what
// would make the lift observable.
//
// Every case here is one the rule leaves alone, and two reasons are mixed on
// purpose: a chain that is not what the closure answers with is not lifted
// whatever precedes it, and a chain that is the answer is not lifted when
// something with an effect runs first. The second is the one these positions
// are about, so each is written with a call before the chain.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

/// An effect, so a call to this is observable where it precedes the chain.
fn record(length: usize) -> usize {
    length
}

fn combine(left: usize, right: usize) -> usize {
    left + right
}

struct Tally(usize);

impl Tally {
    fn absorb(&self, length: usize) -> usize {
        self.0 + length
    }
}

/// Observable for the same reason: reaching the receiver runs this.
fn tally() -> Tally {
    Tally(0)
}

// Not flagged: the chain is an argument rather than what the closure answers
// with, and a step is lifted only out of the answer. Nothing with an effect
// precedes it here, so the position is what declines it, not the ordering.
fn method_argument(counter: Tally, lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(|text| counter.absorb(text.trim().len())).collect()
}

// Not flagged: the receiver of that call is itself a call, which runs before
// the argument holding the chain.
fn method_receiver_runs_first(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(|text| tally().absorb(text.trim().len())).collect()
}

// Not flagged: an argument before the one holding the chain runs first.
fn earlier_argument(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|text| combine(record(0), text.trim().len()))
        .collect()
}

// Not flagged: an element of a tuple before the chain's own runs first.
fn tuple_element(lines: std::vec::IntoIter<&'static str>) -> Vec<(usize, usize)> {
    lines.map(|text| (record(0), text.trim().len())).collect()
}

// Not flagged: and an element of an array, which evaluates the same way.
fn array_element(lines: std::vec::IntoIter<&'static str>) -> Vec<[usize; 2]> {
    lines.map(|text| [record(0), text.trim().len()]).collect()
}

// Not flagged: the left operand of an arithmetic operator runs before the
// right one holding the chain.
fn left_operand(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(|text| record(0) + text.trim().len()).collect()
}

// Not flagged: an indexed base runs before the index holding the chain.
fn indexed_base(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|text| [1_usize, 2, 3, 4][record(0) + text.trim().len() % 4])
        .collect()
}

// Not flagged: an item declared in the block is not an expression that runs,
// but the block holding it is still walked for the ones that do.
fn block_with_an_item(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|text| {
            fn helper() {}
            record(0);
            text.trim().len()
        })
        .collect()
}

// Not flagged: an assignment runs its right side before the place, so a
// chain in the place has the right side before it.
fn assigned_place(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    let mut table = [0_usize; 8];
    lines
        .map(|text| {
            table[text.trim().len() % 8] = record(1);
            0
        })
        .collect()
}

// Not flagged: a compound assignment counts either operand as the one that
// may have run first, so a chain in the place has the value before it.
fn compound_place(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    let mut table = [0_usize; 8];
    lines
        .map(|text| {
            table[text.trim().len() % 8] += record(1);
            0
        })
        .collect()
}

// Not flagged: the chain is the left operand, so the right one runs after it
// and nothing it does could be reordered by the lift. One step is left in the
// closure either way, which is what declines it.
fn right_operand_runs_after(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(|text| text.trim().len() + record(0)).collect()
}

// Not flagged: the chain is the indexed base, so the index runs after it.
fn index_runs_after(lines: std::vec::IntoIter<&'static str>) -> Vec<u8> {
    lines.map(|text| text.trim().as_bytes()[record(0)]).collect()
}

fn main() {}
