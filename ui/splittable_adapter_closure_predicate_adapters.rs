// edition:2024
//
// Which adapters a test may be lifted out of, and what it lifts into. The
// leading adapter drops the items failing it, so one whose answer depends
// on what it was not handed answers differently once split: `all` and
// `is_none_or` go vacuously true, `position` renumbers what it counts, and
// `skip_while` has no target reproducing what it drops. `Result` has no
// filtering adapter to lift into at all.
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

// Bad: a consumer rather than an adapter, whose answer depends only on
// which items satisfy the predicate, so a leading `filter` leaves it the
// same one to find.
fn find_tests(mut lines: std::vec::IntoIter<&'static str>) -> Option<&'static str> {
    lines.find(|line| wanted(line) && line.starts_with('#'))
}

// Good: the leading test filters, leaving the consumer the last one.
fn split_find_tests(lines: std::vec::IntoIter<&'static str>) -> Option<&'static str> {
    lines
        .filter(|line| wanted(line))
        .find(|line| line.starts_with('#'))
}

// Bad: the same from the other end.
fn rfind_tests(mut lines: std::vec::IntoIter<&'static str>) -> Option<&'static str> {
    lines.rfind(|line| wanted(line) && line.starts_with('#'))
}

// Good: the same from the other end, `Filter` being double-ended wherever
// its iterator is.
fn split_rfind_tests(lines: std::vec::IntoIter<&'static str>) -> Option<&'static str> {
    lines
        .filter(|line| wanted(line))
        .rfind(|line| line.starts_with('#'))
}

// Bad: `any` takes its item by value where the lifted `filter` hands a
// reference, and a `&&str` derefs to where these tests read it.
fn any_tests(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(|line| wanted(line) && line.starts_with('#'))
}

// Good: the leading test filters, which hands it one reference more than
// `any` did, and a `&&str` derefs to where the test reads it.
fn split_any_tests(lines: std::vec::IntoIter<&'static str>) -> bool {
    lines
        .filter(|line| wanted(line))
        .any(|line| line.starts_with('#'))
}

// Bad: a prefix-shaped adapter, whose tests lift into a `take_while`
// rather than a `filter`.
fn take_while_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .take_while(|line| wanted(line) && line.starts_with('#'))
        .collect()
}

// Good: one `take_while` per test, which stops the run on the first item
// failing either, as the conjunction did.
fn split_take_while_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .take_while(|line| wanted(line))
        .take_while(|line| line.starts_with('#'))
        .collect()
}

// Not flagged: filtering before `all` makes an item that failed the
// first test vacuously fine, so the answer flips.
fn all_tests(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.all(|line| wanted(line) && line.starts_with('#'))
}

// Not flagged: filtering renumbers what `position` counts.
fn position_tests(mut lines: std::vec::IntoIter<&'static str>) -> Option<usize> {
    lines.position(|line| wanted(line) && line.starts_with('#'))
}

// Not flagged: neither target reproduces what `skip_while` drops.
fn skip_while_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .skip_while(|line| wanted(line) && line.starts_with('#'))
        .collect()
}

// Bad: `Option::filter` keeps whichever values pass rather than stopping at
// one, so its tests lift into a leading `filter` of its own.
fn option_filter(line: Option<&'static str>) -> Option<&'static str> {
    line.filter(|line| wanted(line) && line.starts_with('#'))
}

// Good: one `Option::filter` per test.
fn split_option_filter(line: Option<&'static str>) -> Option<&'static str> {
    line.filter(|line| wanted(line))
        .filter(|line| line.starts_with('#'))
}

// Bad: and so is `Option::is_some_and`.
fn option_is_some_and(line: Option<&'static str>) -> bool {
    line.is_some_and(|line| wanted(line) && line.starts_with('#'))
}

// Good: the leading test filters, leaving the adapter the last one.
fn split_option_is_some_and(line: Option<&'static str>) -> bool {
    line.filter(|line| wanted(line))
        .is_some_and(|line| line.starts_with('#'))
}

// Not flagged: filtering before `is_none_or` makes a value that failed
// the first test vacuously fine, the way it does before `all`.
fn option_is_none_or(line: Option<&'static str>) -> bool {
    line.is_none_or(|line| wanted(line) && line.starts_with('#'))
}

// Not flagged: `Result` has no filtering adapter to lift into, so a
// conjunction on one stays folded however it is written.
fn result_is_ok_and(outcome: Result<&'static str, usize>) -> bool {
    outcome.is_ok_and(|line| wanted(line) && line.starts_with('#'))
}

fn main() {}
