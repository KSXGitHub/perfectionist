// edition:2024
//
// What the split cannot take with it. It makes a second closure, which
// needs its own copy of whatever the first captured, and for the adapters
// that leave their receiver usable it moves that receiver into the leading
// one. So a capture only one closure could hold declines, and so does a
// receiver the function goes on to name -- counting mentions in the body
// that declares it, and counting a mention in a loop as the move it runs
// again.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn wanted(line: &str) -> bool {
    !line.is_empty()
}

fn longer(line: &str, limit: usize) -> bool {
    line.len() > limit
}

fn shorter(line: &str, limit: usize) -> bool {
    line.len() < limit * 2
}

struct Pending {
    rest: std::vec::IntoIter<&'static str>,
}

fn note(log: &mut Vec<usize>, line: &str) -> bool {
    log.push(line.len());
    !line.is_empty()
}

// Bad: a `move` closure's `Copy` capture is one both closures can hold,
// each its own copy, so two tests reaching it still split.
fn copy_capture_in_two_tests(limit: usize, mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(move |line| longer(line, limit) && shorter(line, limit))
}

// Good: each closure holds its own copy of the capture, which is what lets
// both of them read it.
fn split_copy_capture_in_two_tests(
    limit: usize,
    lines: std::vec::IntoIter<&'static str>,
) -> bool {
    lines
        .filter(move |line| longer(line, limit))
        .any(move |line| shorter(line, limit))
}

// Not flagged: two tests reaching a capture held mutably would be two
// closures both holding it.
fn mutable_capture_in_two_tests(lines: std::vec::IntoIter<&'static str>) -> usize {
    let mut log = Vec::new();
    lines
        .filter(|line| note(&mut log, line) && note(&mut log, line.trim()))
        .count()
}

// Not flagged: a receiver the closure does not own cannot be moved into
// the leading `filter`, which `E0507` would say.
fn receiver_is_a_field(pending: &mut Pending) -> bool {
    pending
        .rest
        .any(|line| wanted(line) && line.starts_with('#'))
}

// Not flagged: `find` leaves the receiver where it was and the leading
// `filter` would move it, so a receiver named again is `E0382` once
// split.
fn receiver_named_again(
    mut lines: std::vec::IntoIter<&'static str>,
) -> (Option<&'static str>, Option<&'static str>) {
    let first = lines.find(|line| wanted(line) && line.starts_with('#'));
    (first, lines.next())
}

// Not flagged: one mention is not one move. A call in a loop runs again,
// and the second run moves a receiver the first one took.
fn receiver_moved_twice(mut lines: std::vec::IntoIter<&'static str>) -> usize {
    let mut hits = 0;
    for _ in 0..2 {
        if lines.find(|line| wanted(line) && line.starts_with('#')).is_some() {
            hits += 1;
        }
    }
    hits
}

// Not flagged: and the body to count mentions in is the one declaring the
// receiver, not the closure the call sits in.
fn receiver_named_outside(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    let found = (|| lines.find(|line| wanted(line) && line.starts_with('#')))();
    found.is_some() || lines.next().is_some()
}

fn main() {}
