// edition:2024
//
// Which conjunctions of comparisons read as one test, and which read as
// several.
//
// A comparison is a bound rather than a question, so a conjunction of
// bounds on one quantity asks a single thing. Bounds on two quantities ask
// two, however they are spelled, which is what the rule splits.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

struct Range {
    start: usize,
    end: usize,
}

struct Span {
    offset: usize,
    length: usize,
}

fn wanted(line: &str) -> bool {
    !line.is_empty()
}

// Not flagged: a conjunction of comparisons on one quantity is one test.
// Each of these asks a single question that its halves do not: whether a
// byte is whitespace, and whether a position is inside a range.
fn whitespace_byte(bytes: std::slice::Iter<'static, u8>) -> bool {
    bytes.clone().any(|byte| *byte != b' ' && *byte != b'\t')
}

fn inside_a_range(positions: std::vec::IntoIter<usize>, start: usize, end: usize) -> bool {
    positions.into_iter().any(|pos| pos >= start && pos < end)
}

// Not flagged: which side of the operator the quantity falls on is not
// asked, so the conventional way to spell a range check reads as the one
// test it is.
fn range_check_with_the_sides_swapped(
    positions: std::vec::IntoIter<usize>,
    range: &Range,
) -> bool {
    positions
        .into_iter()
        .any(|pos| range.start <= pos && pos < range.end)
}

// Not flagged: three bounds on one quantity are still one test.
fn three_bounds_on_one_quantity(
    positions: std::vec::IntoIter<usize>,
    start: usize,
    end: usize,
    skipped: usize,
) -> bool {
    positions
        .into_iter()
        .any(|pos| pos >= start && pos < end && pos != skipped)
}

// Not flagged: a quantity read through a call is still one quantity, the
// comparisons bounding the same reading of it.
fn two_bounds_on_a_measured_quantity(
    lines: std::vec::IntoIter<&'static str>,
) -> Vec<&'static str> {
    lines
        .filter(|line| line.len() > 3 && line.len() < 80)
        .collect()
}

// Bad: two fields are two quantities, so the conjunction asks two
// questions and each wants an adapter.
fn bounds_on_two_fields(spans: std::vec::IntoIter<Span>, start: usize, limit: usize) -> Vec<Span> {
    spans
        .filter(|span| span.offset >= start && span.length < limit)
        .collect()
}

// Good: one adapter per quantity bounded.
fn split_bounds_on_two_fields(
    spans: std::vec::IntoIter<Span>,
    start: usize,
    limit: usize,
) -> Vec<Span> {
    spans
        .filter(|span| span.offset >= start)
        .filter(|span| span.length < limit)
        .collect()
}

// Bad: two readings of the item are two quantities, the raw length and the
// trimmed one being different numbers.
fn bounds_on_two_measurements(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| line.len() > 3 && line.trim().len() < 80)
        .collect()
}

// Good: one adapter per reading.
fn split_bounds_on_two_measurements(
    lines: std::vec::IntoIter<&'static str>,
) -> Vec<&'static str> {
    lines
        .filter(|line| line.len() > 3)
        .filter(|line| line.trim().len() < 80)
        .collect()
}

// Bad: a quantity shared by two of three conjuncts is shared by none of
// them, the third asking about something else.
fn a_quantity_shared_by_two_of_three(
    spans: std::vec::IntoIter<Span>,
    start: usize,
    limit: usize,
    skipped: usize,
) -> Vec<Span> {
    spans
        .filter(|span| span.offset >= start && span.length < limit && span.offset != skipped)
        .collect()
}

// Good: one adapter per test, in the order the conjunction asked them.
fn split_a_quantity_shared_by_two_of_three(
    spans: std::vec::IntoIter<Span>,
    start: usize,
    limit: usize,
    skipped: usize,
) -> Vec<Span> {
    spans
        .filter(|span| span.offset >= start)
        .filter(|span| span.length < limit)
        .filter(|span| span.offset != skipped)
        .collect()
}

// Bad: equality is a bound like any other, so two fields compared for
// equality are still two questions.
fn equality_on_two_fields(spans: std::vec::IntoIter<Span>) -> Vec<Span> {
    spans
        .filter(|span| span.offset == 0 && span.length == 1)
        .collect()
}

// Good: one adapter per field.
fn split_equality_on_two_fields(spans: std::vec::IntoIter<Span>) -> Vec<Span> {
    spans
        .filter(|span| span.offset == 0)
        .filter(|span| span.length == 1)
        .collect()
}

// Bad: one comparison among named questions is not a conjunction of
// comparisons. The reader is being asked two things, and the bound is one
// of them rather than half of one.
fn a_comparison_among_calls(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| line.len() > 3 && wanted(line)).collect()
}

// Good: the bound is one of the two questions, so it gets one adapter.
fn split_a_comparison_among_calls(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| line.len() > 3)
        .filter(|line| wanted(line))
        .collect()
}

fn main() {}
