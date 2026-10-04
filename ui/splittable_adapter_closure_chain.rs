// edition:2024
//
// Which closures the rule splits, and which it leaves alone. Where the
// chain sits in the body is its own fixture,
// `splittable_adapter_closure_anchoring.rs`.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.
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

fn maybe(text: &str) -> Option<usize> {
    text.parse().ok()
}


fn bump(seen: &mut Vec<usize>) -> usize {
    seen.push(0);
    seen.len()
}


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


// Bad: a consumer rather than an adapter, whose item still enters by
// value and never comes back out.
fn consumer(lines: std::vec::IntoIter<&'static str>) {
    lines.for_each(|line| record(line.trim().len()));
}


// Good: the steps lifted, leaving the consumer the call that is its own.
fn split_consumer(lines: std::vec::IntoIter<&'static str>) {
    lines.map(str::trim).map(str::len).for_each(record);
}


// Bad: a predicate-returning adapter, where the chain ends in a `bool`.
fn predicate_chain(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(|line| line.trim().is_empty())
}


// Good: the step lifted, leaving the adapter its test. The receiver needs
// no `mut` once the leading `map` takes it by value.
fn split_predicate_chain(lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.map(str::trim).any(str::is_empty)
}


// Bad: the last step stays with the adapter, so only the steps a
// leading `map` would take have to lift. `label`'s result borrows, and
// an owned item could not hand that back out of a `map`.
fn last_step_borrows(headers: std::vec::IntoIter<String>) -> Vec<&'static str> {
    headers.map(|header| label(header.len())).collect()
}


// Good: the step before the last lifted, leaving the adapter the call
// whose result borrows.
fn split_last_step_borrows(headers: std::vec::IntoIter<String>) -> Vec<&'static str> {
    headers.map(|header| header.len()).map(label).collect()
}


// Not flagged: `any` takes `&mut self`, so it leaves the receiver
// positioned and usable, and the leading `map` would move it. A receiver
// named again is `E0382` once split.
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


// Bad: `filter_map`, `find_map` and `map_while` meet the same condition,
// and a body that is a chain rather than `Option` work lifts into a
// leading `map` as any other adapter's does. The guard-and-value trigger
// answers first where the body is `Option` work.
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


// Good: one adapter per step, each of the three adapters left what it is.
fn split_parsing(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(str::trim)
        .map(str::parse::<usize>)
        .filter_map(Result::ok)
        .collect()
}


fn split_first_parsed(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines
        .map(str::trim)
        .map(str::parse::<usize>)
        .find_map(Result::ok)
}


fn split_parsed_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(str::trim)
        .map(str::parse::<usize>)
        .map_while(Result::ok)
        .collect()
}


// Bad: a fallible call applied to a step, which is two steps and no
// `Option` work, so the chain is what splits.
fn parse_the_trimmed(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| maybe(line.trim())).collect()
}


// Good: the step lifted, leaving the adapter the fallible call.
fn split_parse_the_trimmed(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(str::trim).filter_map(maybe).collect()
}


// Bad: the same over items that are `&String`, where what the step borrows
// sits behind the reference and so outlives the closure.
fn parse_the_trimmed_borrowed(lines: std::slice::Iter<'static, String>) -> Vec<usize> {
    lines.filter_map(|line| maybe(line.trim())).collect()
}


// Good: the step lifted. `str::trim` will not do here, a function item
// taking no deref coercion, so the closure stays and holds one step.
fn split_parse_the_trimmed_borrowed(lines: std::slice::Iter<'static, String>) -> Vec<usize> {
    lines.map(|line| line.trim()).filter_map(maybe).collect()
}


// Not flagged: items that own their text, where the lifted step would
// return a borrow of the closure's own parameter and be `E0515`.
fn parse_the_trimmed_owned(lines: std::vec::IntoIter<String>) -> Vec<usize> {
    lines.filter_map(|line| maybe(line.trim())).collect()
}

// Bad: a `&mut` item is a reference the closure was handed, so a step
// borrowing through it lifts. It is not `Copy`, which is why liftability
// asks whether the step borrows through a reference rather than whether
// the item can be copied.
fn mutable_reference_item(rows: std::slice::IterMut<'static, String>) -> Vec<usize> {
    rows.map(|row| row.as_str().len()).collect()
}

// Good: the step lifted, the borrow it hands on being of what the
// reference points at.
fn split_mutable_reference_item(rows: std::slice::IterMut<'static, String>) -> Vec<usize> {
    rows.map(|row| row.as_str()).map(str::len).collect()
}

// Not flagged: a `Copy` item that is not a reference. Copying four bytes
// is cheap and beside the point: the step borrows the closure's own
// parameter, which the lifted `map` ends before the borrow is used, so the
// split is `E0515`.
fn copy_item(rows: std::vec::IntoIter<[u8; 4]>) -> Vec<usize> {
    rows.map(|row| row.as_slice().len()).collect()
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

// Good: the same table, split. Each adapter is left what it is, and the
// stateful ones keep the accumulation alone.
fn split_flattening(lines: std::vec::IntoIter<&'static str>) -> String {
    lines.map(str::trim).flat_map(str::chars).collect()
}

fn split_trying_each(lines: std::vec::IntoIter<&'static str>) -> Option<()> {
    lines
        .map(str::trim)
        .map(str::len)
        .map(record)
        .try_for_each(Some)
}

fn split_every(lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.map(str::trim).all(str::is_empty)
}

fn split_first_empty(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.map(str::trim).position(str::is_empty)
}

fn split_last_empty(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.map(str::trim).rposition(str::is_empty)
}

fn split_try_total(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines
        .map(str::trim)
        .map(str::len)
        .try_fold(0, |total, length| Some(total + length))
}

fn split_total_from_the_right(lines: std::vec::IntoIter<&'static str>) -> usize {
    lines
        .map(str::trim)
        .map(str::len)
        .rfold(0, |total, length| total + length)
}

fn split_try_total_from_the_right(lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines
        .map(str::trim)
        .map(str::len)
        .try_rfold(0, |total, length| Some(total + length))
}

fn split_running_total(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(str::trim)
        .map(str::len)
        .scan(0, |total, length| Some(*total + length))
        .collect()
}


// Not flagged: the second step is applied to an owned `String`, which
// dies at the end of the `map` it would lift into, however the item
// arrived.
fn owned_intermediate(lines: std::vec::IntoIter<&'static str>) -> Vec<String> {
    lines
        .map(|line| line.to_lowercase().trim().to_string())
        .collect()
}


// Not flagged: a step mutably borrowing a capture the closure holds
// too, which two closures could not both do.
fn mutable_capture_in_a_step(lines: std::vec::IntoIter<&'static str>) -> usize {
    let mut seen = Vec::new();
    lines.fold(0, |total, line| {
        total + line.trim().len().min(bump(&mut seen))
    })
}


// Bad: a shared capture is one two closures may both hold, so a step
// reaching it still splits.
fn shared_capture_in_a_step(limit: usize, lines: std::vec::IntoIter<&'static str>) -> usize {
    lines.fold(0, |total, line| total + line.trim().len().min(limit))
}

// Good: the steps lifted, the capture going with the step that reached it.
fn split_shared_capture_in_a_step(
    limit: usize,
    lines: std::vec::IntoIter<&'static str>,
) -> usize {
    lines
        .map(str::trim)
        .map(str::len)
        .map(|length| length.min(limit))
        .fold(0, |total, length| total + length)
}


// Bad: a step whose result cannot cross a thread, which only a parallel
// adapter asks of it.
fn not_sendable_sequentially(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| std::rc::Rc::new(line).len())
        .collect()
}

// Good: the step lifted, which a sequential adapter asks nothing of. One
// `Rc` per item either way.
fn split_not_sendable_sequentially(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(std::rc::Rc::new)
        .map(|held| held.len())
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


// Bad: an argument that is a parameter of the method's own lends the
// result nothing, however the call instantiates it. `trim_start_matches`
// takes a `P: Pattern`, so the `&str` passed here carries a region the
// erased types cannot tell from the result's, where the declared signature
// can.
fn trimmed_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| line.trim_start_matches("# ").len())
        .collect()
}

// Good: the step lifted, its pattern argument going with it.
fn split_trimmed_prefix(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| line.trim_start_matches("# "))
        .map(str::len)
        .collect()
}


struct Lender(&'static str);

impl Lender {
    fn pick<'a>(&'a self, fallback: &'a str) -> &'a str {
        match self.0.is_empty() {
            true => fallback,
            false => self.0,
        }
    }
}


// Not flagged: this signature gives the result the argument's own region,
// so what the result borrows may be the temporary the closure made. That
// is the other half of what the declared signature answers.
fn region_from_an_argument(rows: std::slice::Iter<'static, Lender>) -> Vec<usize> {
    rows.map(|row| row.pick(&String::from("m")).len()).collect()
}


// Not flagged: the step's result borrows the shorter of two lifetimes,
// and the shorter one is a temporary this closure made, so the receiver
// being a reference does not answer for it.
fn argument_lends_the_result(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| line.max(&String::from("m")[..]).len())
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


macro_rules! trimmed_lengths {
    ($lines:expr) => {
        $lines.map(|line| line.trim().len()).collect::<Vec<_>>()
    };
}


// Not flagged: the adapter comes from the expansion.
fn built_from_a_macro(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    trimmed_lengths!(lines)
}


// Not flagged: a `ref` item types as a reference to the closure's own
// parameter slot, so a step borrowing through it borrows what dies with
// the closure.
fn ref_item(headers: std::vec::IntoIter<String>) -> Vec<usize> {
    headers.map(|ref header| header.trim().len()).collect()
}


// Not flagged: a callee that is not a path can hand back an `Fn` holding
// a borrow, which the step's result then carries.
fn pick<'chosen>(prefix: &'chosen str) -> impl Fn(&str) -> &'chosen str {
    move |_| prefix
}


fn callee_lends_the_result(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(|line| pick(&String::from("m"))(line).len())
        .collect()
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


// Not flagged: a `move` closure writing to a `Copy` capture would give
// each half its own copy, which compiles and answers differently.
fn move_closure_writes_its_capture(lines: std::vec::IntoIter<&'static str>) -> usize {
    let mut seen = 0usize;
    lines.fold(0, move |total, line| {
        seen += 1;
        total + line.trim().len().min(seen)
    })
}


// Bad: a `move` closure holds its own copy of a `Copy` capture, so two of
// them may both read it.
fn move_closure_copies_its_capture(
    limit: usize,
    lines: std::vec::IntoIter<&'static str>,
) -> usize {
    lines.fold(0, move |total, line| {
        total + line.trim().len().min(limit)
    })
}

// Good: the steps lifted, each closure holding its own copy of the
// capture, which is what lets both of them have it.
fn split_move_closure_copies_its_capture(
    limit: usize,
    lines: std::vec::IntoIter<&'static str>,
) -> usize {
    lines
        .map(str::trim)
        .map(str::len)
        .map(move |length| length.min(limit))
        .fold(0, |total, length| total + length)
}


fn main() {}
