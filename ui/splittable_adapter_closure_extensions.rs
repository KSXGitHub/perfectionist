// aux-build:rayon_stub.rs
// aux-build:pipe_trait_stub.rs
// edition:2024
//
// The extension traits, whose paths are what identify them: none carries
// a diagnostic item.
//
// `itertools` needs no stub: the driver's sysroot ships the real crate,
// because rustc itself depends on it, and a stub of that name collides
// with it (`E0464`). Reaching it from there is what `rustc_private` is
// for. rayon and `pipe-trait` are not in the sysroot, so each has a
// stub.
//
// Every Prefer form the rule's own docs ask for is here too. A form the
// rule suggests and then fires on again is a false positive that reading
// the Avoid cases alone would never find.

#![feature(register_tool, rustc_private)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate itertools;
extern crate pipe_trait;
extern crate rayon;

use itertools::Itertools;
use pipe_trait::Pipe;
use rayon::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;

fn record(length: usize) {}

fn double(value: usize) -> usize {
    value * 2
}

fn render(value: usize) -> String {
    value.to_string()
}

fn wanted(text: &str) -> bool {
    !text.is_empty()
}

fn classify(text: &'static str) -> itertools::Either<usize, &'static str> {
    match text.len() > 3 {
        true => itertools::Either::Left(text.len()),
        false => itertools::Either::Right(text),
    }
}

fn results() -> std::vec::IntoIter<Result<&'static str, usize>> {
    vec![Ok(" a "), Err(7)].into_iter()
}

fn lines() -> std::vec::IntoIter<&'static str> {
    vec![" a ", " bb "].into_iter()
}

// Bad: the `Ok` inside a `Result` item is a channel nested one level
// inside the iterator's own, so a lifted step goes into `map_ok`.
fn nested_channel(
    items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Vec<Result<usize, usize>> {
    items.map_ok(|text| text.trim().len()).collect()
}

// Good: one adapter per step, on the nested channel.
fn split_nested_channel(
    items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Vec<Result<usize, usize>> {
    items.map_ok(str::trim).map_ok(str::len).collect()
}

// Bad: binary over that same nested item.
fn nested_fold(
    mut items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Result<usize, usize> {
    items.fold_ok(0, |total, text| total + text.trim().len())
}

// Good: the steps lifted into the adapter mapping that channel, leaving
// the closure the accumulation.
fn split_nested_fold(items: std::vec::IntoIter<Result<&'static str, usize>>) -> Result<usize, usize> {
    items
        .map_ok(str::trim)
        .map_ok(str::len)
        .fold_ok(0, |total, length| total + length)
}

// Bad: binary over the iterator's own item, so this one lifts into `map`.
fn own_fold(mut items: std::vec::IntoIter<&'static str>) -> usize {
    items
        .fold_while(0, |total, text| {
            itertools::FoldWhile::Continue(total + text.trim().len())
        })
        .into_inner()
}

// Good: the steps lifted into `map`, this adapter's item being the
// iterator's own.
fn split_own_fold(items: std::vec::IntoIter<&'static str>) -> usize {
    items
        .map(str::trim)
        .map(str::len)
        .fold_while(0, |total, length| itertools::FoldWhile::Continue(total + length))
        .into_inner()
}

// Bad: a parallel adapter, which splits the way the sequential one
// does.
fn parallel(items: Parallel<&'static str>) -> Parallel<usize> {
    items.map(|text| text.trim().len())
}

// Good: the same split.
fn split_parallel(items: Parallel<&'static str>) -> Parallel<usize> {
    items.map(str::trim).map(str::len)
}

// Bad: a parallel predicate-returning consumer.
fn parallel_any(items: Parallel<&'static str>) -> bool {
    items.any(|text| text.trim().is_empty())
}

// Good: the step lifted, leaving the consumer its test.
fn split_parallel_any(items: Parallel<&'static str>) -> bool {
    items.map(str::trim).any(str::is_empty)
}

// Bad: a piping method, where every step becomes one and none stays.
fn piped(value: usize) -> String {
    value.pipe(|number| render(double(number)))
}

// Good: one `pipe` per step.
fn split_piped(value: usize) -> String {
    value.pipe(double).pipe(render)
}

// Bad: a parallel predicate that is a conjunction, which the conjunction
// trigger splits into successive `filter`s.
fn parallel_conjunction(items: Parallel<&'static str>) -> bool {
    items.any(|text| wanted(text) && text.starts_with('#'))
}

// Good: one adapter per test, the leading one filtering.
fn split_parallel_conjunction(items: Parallel<&'static str>) -> bool {
    items
        .filter(|text| wanted(text))
        .any(|text| text.starts_with('#'))
}

// Not flagged: a parallel `filter` hands the item back downstream, the
// way the sequential one does.
fn parallel_filter(items: Parallel<&'static str>) -> Parallel<&'static str> {
    items.filter(|text| text.trim().is_empty())
}

// Not flagged: a lifted step's result has to cross a thread, and an `Rc`
// cannot. The folded form compiles because the value never leaves the
// closure.
fn not_sendable(items: Parallel<&'static str>) -> Parallel<usize> {
    items.map(|text| Rc::new(text).len())
}

// Not flagged: `pipe` moves the receiver into the closure, so a step
// returning a borrow of it dies with the closure.
fn piped_borrowing(value: String) -> usize {
    value.pipe(|owned| owned.trim().len())
}

// Bad: `pipe_ref` hands the closure a shared borrow, so a step borrowing
// through it outlives the closure and lifts.
fn piped_by_reference(value: String) -> usize {
    value.pipe_ref(|text| text.trim().len())
}

// Good: the head keeps the method, which is what keeps the receiver
// borrowed rather than moved.
fn split_piped_by_reference(value: String) -> usize {
    value.pipe_ref(|text| text.trim()).pipe(str::len)
}

// Bad: three steps rather than two, so the lifting runs past one step.
fn piped_by_reference_thrice(value: String) -> usize {
    value.pipe_ref(|text| text.trim().to_uppercase().len())
}

// Good: the head keeps the method, and both steps above it take the value a
// `pipe` hands them.
fn split_piped_by_reference_thrice(value: String) -> usize {
    value
        .pipe_ref(|text| text.trim())
        .pipe(str::to_uppercase)
        .pipe(|upper| upper.len())
}

// Bad: `pipe_mut` hands the closure a mutable borrow, which a step may
// borrow through just as it may a shared one.
fn piped_mutably(mut value: Vec<usize>) -> usize {
    value.pipe_mut(|all| all.as_mut_slice().len())
}

// Good: the head keeps `pipe_mut`, which is what keeps the borrow mutable.
fn split_piped_mutably(mut value: Vec<usize>) -> usize {
    value.pipe_mut(|all| all.as_mut_slice()).pipe(|all| all.len())
}

// Bad: `pipe_as_ref` converts through `AsRef` before handing the borrow
// over, and the step borrows through what the conversion produced.
fn piped_as_reference(value: String) -> usize {
    value.pipe_as_ref(|text: &str| text.trim().len())
}

// Good: the head keeps `pipe_as_ref`, so the conversion still happens once.
fn split_piped_as_reference(value: String) -> usize {
    value.pipe_as_ref(|text: &str| text.trim()).pipe(str::len)
}

// Bad: `pipe_as_mut` converts through `AsMut`, the mutable counterpart of
// the same conversion.
fn piped_as_mutable(mut value: String) -> usize {
    value.pipe_as_mut(|text: &mut str| text.trim().len())
}

// Good: the head keeps `pipe_as_mut`, so the conversion still happens once.
fn split_piped_as_mutable(mut value: String) -> usize {
    value.pipe_as_mut(|text: &mut str| text.trim()).pipe(str::len)
}

// Bad: `pipe_deref` reaches the borrow through `Deref` rather than a
// conversion the caller names.
fn piped_through_deref(value: String) -> usize {
    value.pipe_deref(|text: &str| text.trim().len())
}

// Good: the head keeps `pipe_deref`, so the deref still happens once.
fn split_piped_through_deref(value: String) -> usize {
    value.pipe_deref(|text: &str| text.trim()).pipe(str::len)
}

// Bad: `pipe_deref_mut` reaches a mutable borrow through `DerefMut`.
fn piped_through_deref_mut(mut value: String) -> usize {
    value.pipe_deref_mut(|text: &mut str| text.trim().len())
}

// Good: the head keeps `pipe_deref_mut`, so the deref still happens once.
fn split_piped_through_deref_mut(mut value: String) -> usize {
    value.pipe_deref_mut(|text: &mut str| text.trim()).pipe(str::len)
}

// Bad: `pipe_borrow` reaches the borrow through `Borrow`, which a type may
// implement for several targets.
fn piped_through_borrow(value: String) -> usize {
    value.pipe_borrow(|text: &str| text.trim().len())
}

// Good: the head keeps `pipe_borrow`, so the borrow is still of the target
// the annotation picked.
fn split_piped_through_borrow(value: String) -> usize {
    value.pipe_borrow(|text: &str| text.trim()).pipe(str::len)
}

// Bad: `pipe_borrow_mut` reaches a mutable borrow through `BorrowMut`.
fn piped_through_borrow_mut(mut value: String) -> usize {
    value.pipe_borrow_mut(|text: &mut str| text.trim().len())
}

// Good: the head keeps `pipe_borrow_mut`, so the borrow is still mutable
// and still of the target the annotation picked.
fn split_piped_through_borrow_mut(mut value: String) -> usize {
    value.pipe_borrow_mut(|text: &mut str| text.trim()).pipe(str::len)
}

// Bad: a receiver named again after the call, which the split borrows
// rather than moves, so there is nothing for the receiver-move gate to
// decline.
fn piped_by_reference_then_used(value: String) -> (usize, String) {
    let length = value.pipe_ref(|text| text.trim().len());
    (length, value)
}

// Good: the split borrows the receiver where the folded form did, so the
// name is still there afterwards.
fn split_piped_by_reference_then_used(value: String) -> (usize, String) {
    let length = value.pipe_ref(|text| text.trim()).pipe(str::len);
    (length, value)
}

// Bad: `filter_map_ok`'s closure returns an `Option`, and the
// guard-and-value trigger has no split for that on the `Ok` channel, so
// the chain is what is left to read.
fn nested_filter_map(
    items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Vec<Result<usize, usize>> {
    items
        .filter_map_ok(|text| text.trim().parse().ok())
        .collect()
}

// Good: every step but the last lifted into `map_ok`, leaving the adapter
// the call that discards the error.
fn split_nested_filter_map(
    items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Vec<Result<usize, usize>> {
    items
        .map_ok(str::trim)
        .map_ok(str::parse::<usize>)
        .filter_map_ok(Result::ok)
        .collect()
}

// Not flagged: one step is already one adapter doing one thing.
fn one_step(
    items: std::vec::IntoIter<Result<&'static str, usize>>,
) -> Vec<Result<&'static str, usize>> {
    items.map_ok(|text| text.trim()).collect()
}

// Bad: `counts_by` keys a tally by what its closure answers, and the item
// enters it by value.
fn counting(lines: std::vec::IntoIter<&'static str>) -> HashMap<usize, usize> {
    lines.counts_by(|text| text.trim().len())
}

// Good: one adapter per step, `counts_by` left the tallying.
fn split_counting(lines: std::vec::IntoIter<&'static str>) -> HashMap<usize, usize> {
    lines.map(str::trim).counts_by(str::len)
}

// Bad: `partition_map` sends each item one way or the other by what its
// closure answers, the item entering by value.
fn partitioned(lines: std::vec::IntoIter<&'static str>) -> (Vec<usize>, Vec<&'static str>) {
    lines.partition_map(|text| classify(text.trim()))
}

// Good: one adapter per step, `partition_map` left the sorting into two.
fn split_partitioned(
    lines: std::vec::IntoIter<&'static str>,
) -> (Vec<usize>, Vec<&'static str>) {
    lines.map(str::trim).partition_map(classify)
}

// Not flagged: the guard-and-value trigger reads `Iterator`'s own
// `filter_map` alone, and what each combinator hands its work to was tabled
// for `Iterator`, so rayon's is left to the chain trigger.
fn parallel_guarded(items: Parallel<&'static str>) -> Vec<usize> {
    items
        .filter_map(|text| wanted(text).then(|| text.len()))
        .into_items()
}

// Bad: rayon's `for_each` consumes in parallel, and its item enters by
// value and never comes back out.
fn parallel_each(items: Parallel<&'static str>) {
    items.for_each(|text| drop(text.trim().len()));
}

// Good: one adapter per step, `for_each` left the consuming.
fn split_parallel_each(items: Parallel<&'static str>) {
    items.map(str::trim).map(str::len).for_each(drop);
}

// Bad: rayon's `all` asks one test of every item, in whatever order the
// pool reaches them.
fn parallel_every(items: Parallel<&'static str>) -> bool {
    items.all(|text| text.trim().is_empty())
}

// Good: one adapter per step, `all` left the test.
fn split_parallel_every(items: Parallel<&'static str>) -> bool {
    items.map(str::trim).all(str::is_empty)
}

// Bad: `position_any` answers with the index of whichever passing item the
// pool reaches first, and a leading `map` keeps one item per item, so the
// indices are the ones they were.
fn parallel_position(items: Parallel<&'static str>) -> Option<usize> {
    items.position_any(|text| text.trim().is_empty())
}

// Good: one adapter per step, `position_any` left the answering.
fn split_parallel_position(items: Parallel<&'static str>) -> Option<usize> {
    items.map(str::trim).position_any(str::is_empty)
}

// Bad: `position_first` pins the answer to the earliest passing item
// rather than the earliest reached, the indices still one per item.
fn parallel_position_first(items: Parallel<&'static str>) -> Option<usize> {
    items.position_first(|text| text.trim().is_empty())
}

// Good: one adapter per step, `position_first` left the answering.
fn split_parallel_position_first(items: Parallel<&'static str>) -> Option<usize> {
    items.map(str::trim).position_first(str::is_empty)
}

// Bad: `position_last` pins the answer to the latest passing item, and
// counts the same items the folded form counted.
fn parallel_position_last(items: Parallel<&'static str>) -> Option<usize> {
    items.position_last(|text| text.trim().is_empty())
}

// Good: one adapter per step, `position_last` left the answering.
fn split_parallel_position_last(items: Parallel<&'static str>) -> Option<usize> {
    items.map(str::trim).position_last(str::is_empty)
}

// Bad: `find_map_any` takes whichever answer the pool produces first, and
// its item enters by value.
fn parallel_found_any(items: Parallel<&'static str>) -> Option<usize> {
    items.find_map_any(|text| text.trim().parse().ok())
}

// Good: one adapter per step, `find_map_any` left the discarding of errors.
fn split_parallel_found_any(items: Parallel<&'static str>) -> Option<usize> {
    items
        .map(str::trim)
        .map(str::parse::<usize>)
        .find_map_any(Result::ok)
}

// Bad: `find_map_first` takes the answer belonging to the earliest item
// rather than the earliest produced.
fn parallel_found_first(items: Parallel<&'static str>) -> Option<usize> {
    items.find_map_first(|text| text.trim().parse().ok())
}

// Good: one adapter per step, `find_map_first` left the discarding of
// errors.
fn split_parallel_found_first(items: Parallel<&'static str>) -> Option<usize> {
    items
        .map(str::trim)
        .map(str::parse::<usize>)
        .find_map_first(Result::ok)
}

// Bad: `find_map_last` takes the answer belonging to the latest item.
fn parallel_found_last(items: Parallel<&'static str>) -> Option<usize> {
    items.find_map_last(|text| text.trim().parse().ok())
}

// Good: one adapter per step, `find_map_last` left the discarding of
// errors.
fn split_parallel_found_last(items: Parallel<&'static str>) -> Option<usize> {
    items
        .map(str::trim)
        .map(str::parse::<usize>)
        .find_map_last(Result::ok)
}

// Bad: rayon's `fold` takes an accumulator per thread, so it is handed a
// closure whose *second* parameter is the item.
fn parallel_total(items: Parallel<&'static str>) -> Parallel<usize> {
    items.fold(|| 0, |total, text| total + text.trim().len())
}

// Good: one adapter per step, `fold` left the accumulation alone.
fn split_parallel_total(items: Parallel<&'static str>) -> Parallel<usize> {
    items
        .map(str::trim)
        .map(str::len)
        .fold(|| 0, |total, length| total + length)
}

// Bad: rayon's `try_fold` can stop early, its item likewise the second
// parameter of the closure it takes.
fn parallel_try_total(items: Parallel<&'static str>) -> Parallel<Option<usize>> {
    items.try_fold(|| 0, |total, text| Some(total + text.trim().len()))
}

// Good: one adapter per step, `try_fold` left the accumulation alone.
fn split_parallel_try_total(items: Parallel<&'static str>) -> Parallel<Option<usize>> {
    items
        .map(str::trim)
        .map(str::len)
        .try_fold(|| 0, |total, length| Some(total + length))
}

// Bad: `find_first` hands the closure a borrow of the item, which a leading
// `map` would replace, so the chain trigger leaves it alone and the
// conjunction trigger's `filter`, taking the item the same way, reports.
fn parallel_first(items: Parallel<&'static str>) -> Option<&'static str> {
    items.find_first(|text| wanted(text) && text.starts_with('#'))
}

// Good: one adapter per test, the leading `filter` handing the item the way
// `find_first` did.
fn split_parallel_first(items: Parallel<&'static str>) -> Option<&'static str> {
    items
        .filter(|text| wanted(text))
        .find_first(|text| text.starts_with('#'))
}

// Bad: `find_any` hands the closure a borrow too, so the conjunction
// trigger is the one with a split for it.
fn parallel_found(items: Parallel<&'static str>) -> Option<&'static str> {
    items.find_any(|text| wanted(text) && text.starts_with('#'))
}

// Good: one adapter per test, the leading `filter` handing the item the way
// `find_any` did.
fn split_parallel_found(items: Parallel<&'static str>) -> Option<&'static str> {
    items
        .filter(|text| wanted(text))
        .find_any(|text| text.starts_with('#'))
}

fn main() {}
