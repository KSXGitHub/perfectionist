// edition:2024
//
// What the two closures a split makes cannot carry between them: an item
// a guard moves, a capture both halves reach, an item that does not
// survive the extra reference a `filter` hands it, a receiver named after
// the adapter, and a stage whose result borrows.
//
// Where the guard-and-value split is declined the chain trigger may still
// have one, so a Bad case here is followed by the Good form of whichever
// trigger reported it.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn parse(line: &str) -> Option<usize> {
    line.trim().parse().ok()
}

fn validate(value: usize) -> Option<usize> {
    (value > 0).then_some(value)
}

fn render(line: &str) -> usize {
    line.len()
}

fn consume(name: String) -> bool {
    !name.is_empty()
}

fn note(log: &mut Vec<usize>, line: &str) -> bool {
    log.push(line.len());
    true
}

fn record(log: &mut Vec<usize>, line: &str) -> usize {
    log.push(0);
    line.len()
}

#[derive(Clone, Copy)]
struct Tick {
    count: usize,
}

impl Tick {
    fn bump(&mut self) -> bool {
        self.count += 1;
        true
    }
}

// Bad: the guard-and-value split lifts a guard into a `filter`, which
// hands the item by reference, so a guard that moves it is `E0308`. The
// chain trigger's leading `map` hands it over, so the two calls split that
// way instead.
fn guard_moves_the_item(names: std::vec::IntoIter<String>) -> Vec<usize> {
    names.filter_map(|name| consume(name).then_some(1)).collect()
}

// Good: the chain's split, whose leading `map` hands the item over.
fn split_guard_moves_the_item(names: std::vec::IntoIter<String>) -> Vec<usize> {
    names
        .map(consume)
        .filter_map(|flag| flag.then_some(1))
        .collect()
}

// Bad: a `map_while` guard lifts into a `take_while`, which hands the item
// by reference where `map_while` hands it over.
fn prefix_guard_moves_the_item(names: std::vec::IntoIter<String>) -> Vec<usize> {
    names.map_while(|name| consume(name).then_some(1)).collect()
}

// Good: one adapter per half, the guard in the `take_while` that stops the
// run where the folded form stopped it.
fn split_prefix_guard_moves_the_item(names: std::vec::IntoIter<String>) -> Vec<usize> {
    names
        .map(consume)
        .map_while(|flag| flag.then_some(1))
        .collect()
}

// Not flagged: both halves of the split reach the same capture held
// mutably, which two closures cannot do.
fn both_halves_reach_a_capture(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    let mut log = Vec::new();
    lines
        .filter_map(|line| note(&mut log, line).then(|| record(&mut log, line)))
        .collect()
}

// Not flagged: a `Copy` item survives the extra reference a `filter`
// hands it, but one the guard writes to has nothing to write through.
fn copy_item_the_guard_writes_to(ticks: std::vec::IntoIter<Tick>) -> Vec<usize> {
    ticks
        .filter_map(|mut tick| tick.bump().then_some(tick.count))
        .collect()
}

// Not flagged: `find_map` leaves the receiver where it was and the
// leading `filter_map` would move it, so a receiver named again is
// `E0382` once split.
fn receiver_named_again(
    mut lines: std::vec::IntoIter<&'static str>,
) -> (Option<usize>, Option<&'static str>) {
    let first = lines.find_map(|line| parse(line).and_then(validate));
    (first, lines.next())
}

// Not flagged: the guard reads a `Copy` item by value, where the lifted
// `filter` hands a reference and `&T` is no coercion to `T`.
fn copy_item_read_by_value(counts: std::vec::IntoIter<usize>) -> Vec<usize> {
    counts
        .filter_map(|count| (count > 0).then(|| count * 2))
        .collect()
}

fn first_word(text: &str) -> Option<&str> {
    text.split(' ').next()
}

// Not flagged: the stage that stays becomes the next adapter's item, so a
// result borrowing anything leaves the closure with it.
fn stage_borrows(names: std::vec::IntoIter<String>) -> Vec<usize> {
    names
        .filter_map(|name| first_word(&name).map(render))
        .collect()
}

// Bad: this stage borrows what the item points at, which outlives the
// closure, so the chain trigger's split compiles. The guard-and-value
// trigger declines it all the same, telling that borrow from one of a local
// the closure made needing the regions the erased types no longer carry.
fn stage_borrows_the_item(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| line.strip_prefix("# ").map(str::len))
        .collect()
}

// Good: the chain's split, which the borrow outliving the closure lets
// compile.
fn split_stage_borrows_the_item(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| line.strip_prefix("# "))
        .filter_map(|held| held.map(str::len))
        .collect()
}

fn main() {}
