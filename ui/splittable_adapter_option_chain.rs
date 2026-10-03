// edition:2024
//
// Which `Option`-returning closures the rule splits, and which it
// leaves alone.
//
// Every Prefer form the rule's own docs ask for is here too. A form the
// rule suggests and then fires on again is a false positive that reading
// the Avoid cases alone would never find.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn parse(line: &str) -> Option<usize> {
    line.trim().parse().ok()
}

fn validate(value: usize) -> Option<usize> {
    (value > 0).then_some(value)
}

fn double(value: usize) -> usize {
    value * 2
}

fn positive(value: &usize) -> bool {
    *value > 0
}

fn wanted(line: &str) -> bool {
    !line.is_empty()
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

// Bad: two fallible stages welded together.
fn fallible(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| parse(line).and_then(validate)).collect()
}

// Good: one adapter per stage, which is what the rule asks for.
fn split_fallible(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(parse).filter_map(validate).collect()
}

// Bad: a fallible stage and an infallible one.
fn infallible(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| parse(line).map(double)).collect()
}

// Good: the same with the infallible half trailing.
fn split_infallible(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(parse).map(double).collect()
}

// Bad: a fallible stage and a test.
fn filtering(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| parse(line).filter(positive)).collect()
}

// Good: the same with the test trailing.
fn split_filtering(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(parse).filter(positive).collect()
}

// Bad: a guard welded to a value, which is where the rule's name comes
// from. The value names the item, because a `map` over the item is where
// it goes.
fn guarded(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| wanted(line).then(|| render(line))).collect()
}

// Good: the guard filters and the value maps.
fn split_guarded(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter(|line| wanted(line)).map(|line| render(line)).collect()
}

// Bad: `then_some` is the same guard with the value already in hand.
fn guarded_by_value(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| wanted(line).then_some(1)).collect()
}

// Bad: a consumer, whose answer still depends only on which items
// satisfy the guard.
fn found(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(|line| parse(line).and_then(validate))
}

// Bad: a prefix-shaped adapter takes the guard, whose lift target is a
// `take_while`.
fn guarded_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map_while(|line| wanted(line).then(|| render(line))).collect()
}

// Good: the guard stops the run and the value maps.
fn split_guarded_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.take_while(|line| wanted(line)).map(|line| render(line)).collect()
}

// Bad: a trailing `map` drops nothing, so it suits a prefix-shaped
// adapter too.
fn infallible_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map_while(|line| parse(line).map(double)).collect()
}

// Not flagged: a leading `filter_map` would drop the very item that
// would have stopped the run, so this one does not split.
fn fallible_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map_while(|line| parse(line).and_then(validate))
        .collect()
}

// Not flagged: a trailing `filter` moves where the run stops, for the
// same reason.
fn filtering_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map_while(|line| parse(line).filter(positive))
        .collect()
}

// Not flagged: `find_map` yields one value, so the only trailing
// `filter` it has is the `Option`'s, which rejects what the search
// settled on where the folded form kept looking.
fn found_filtering(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(|line| parse(line).filter(positive))
}

// Not flagged: nor has it anywhere to put a guard's value, the one value
// being gone once a leading adapter has made a stream of them.
fn found_guarded(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(|line| wanted(line).then(|| render(line)))
}

// Bad: a trailing `map` drops nothing, so a one-value adapter takes it.
fn found_infallible(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.find_map(|line| parse(line).map(double))
}

// Not flagged: a guard lifts into a `filter`, which hands the item by
// reference, so a guard that moves it is `E0308` once lifted.
fn guard_moves_the_item(names: std::vec::IntoIter<String>) -> Vec<usize> {
    names.filter_map(|name| consume(name).then_some(1)).collect()
}

// Not flagged: and a `map_while` guard lifts into a `take_while`, which
// hands it by reference too.
fn prefix_guard_moves_the_item(names: std::vec::IntoIter<String>) -> Vec<usize> {
    names.map_while(|name| consume(name).then_some(1)).collect()
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

macro_rules! staged {
    ($line:expr) => {
        parse($line).map(double)
    };
}

// Not flagged: a combinator the reader did not write has no stage
// boundary they can move.
fn stage_from_a_macro(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| staged!(line)).collect()
}

// Not flagged: one stage has nothing to hand over.
fn one_stage(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| parse(line)).collect()
}

// Not flagged: the second stage names the item, which the adapter it
// would move to never sees.
fn second_stage_names_the_item(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| parse(line).map(|value| value + line.len()))
        .collect()
}

// Not flagged: a guard naming no item holds for every item or none.
fn invariant_guard(flag: bool, lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| flag.then_some(1)).collect()
}

struct Sieve(&'static str);

impl Sieve {
    fn and_then<Output>(self, body: impl FnOnce(&'static str) -> Option<Output>) -> Option<Output> {
        body(self.0)
    }

    fn then_some<Output>(self, value: Output) -> Option<Output> {
        Some(value)
    }

    fn map<Output>(self, body: impl FnOnce(&'static str) -> Output) -> Option<Output> {
        Some(body(self.0))
    }

    fn filter(self, test: impl FnOnce(&&'static str) -> bool) -> Option<&'static str> {
        test(&self.0).then_some(self.0)
    }
}

fn sieve(line: &'static str) -> Sieve {
    Sieve(line)
}

// Not flagged: `and_then` on something that is not an `Option` is a
// method of that name rather than the combinator.
fn method_of_that_name(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| sieve(line).and_then(parse)).collect()
}

// Not flagged: nor is `then_some` on something that is not a `bool`.
fn guard_of_that_name(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| sieve(line).then_some(line.len()))
        .collect()
}

trait Winnow {
    fn filter_map<Output>(self, body: impl FnOnce(&'static str) -> Option<Output>)
    -> Option<Output>;
}

impl Winnow for &'static str {
    fn filter_map<Output>(
        self,
        body: impl FnOnce(&'static str) -> Option<Output>,
    ) -> Option<Output> {
        body(self)
    }
}

// Not flagged: a trait of one's own, whose `filter_map` says nothing
// about how the value arrives.
fn another_trait_filter_map(line: &'static str) -> Option<usize> {
    line.filter_map(|text| parse(text).and_then(validate))
}

// Not flagged: nor are `map` and `filter` on something that is not an
// `Option`.
fn map_of_that_name(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| sieve(line).map(render)).collect()
}

fn filter_of_that_name(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter_map(|line| sieve(line).filter(|text| wanted(text)))
        .collect()
}

// Bad for the chain rule and not for this one: `Iterator::map` has no
// `Option` work to hand over, so only the steps split.
fn plain_map(lines: std::vec::IntoIter<&'static str>) -> Vec<Option<usize>> {
    lines.map(|line| parse(line).map(double)).collect()
}

// Not flagged: the closure's outermost call takes no argument, so there
// is no second stage to hand over.
fn no_second_stage(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .filter_map(|line| line.trim().parse::<usize>().map(double).ok())
        .collect()
}

fn main() {}
