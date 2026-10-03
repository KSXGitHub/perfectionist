// edition:2024
//
// Which predicates the rule splits, and which it leaves alone.
//
// Every Prefer form the rule's own docs ask for is here too. A form the
// rule suggests and then fires on again is a false positive that reading
// the Avoid cases alone would never find.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

fn wanted(line: &str) -> bool {
    !line.is_empty()
}

// Bad: two tests on the item.
fn two_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) && line.starts_with('#'))
        .collect()
}

// Good: one adapter per test, which is what the rule asks for.
fn split_pair(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line))
        .filter(|line| line.starts_with('#'))
        .collect()
}

// Bad: three tests, so the count in the message is not always two.
fn three_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) && line.starts_with('#') && line.ends_with('!'))
        .collect()
}

// Bad: a disjunction inside a conjunct stays whole, and the pair still
// splits.
fn conjunct_holds_a_disjunction(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) && (line.starts_with('#') || line.ends_with('!')))
        .collect()
}

// Bad: a consumer whose answer still depends only on which items
// satisfy the predicate.
fn find_tests(mut lines: std::vec::IntoIter<&'static str>) -> Option<&'static str> {
    lines.find(|line| wanted(line) && line.starts_with('#'))
}

// Bad: the same from the other end.
fn rfind_tests(mut lines: std::vec::IntoIter<&'static str>) -> Option<&'static str> {
    lines.rfind(|line| wanted(line) && line.starts_with('#'))
}

// Bad: `any` takes its item by value, which bears on the binding the
// rewrite writes rather than on whether it splits.
fn any_tests(mut lines: std::vec::IntoIter<&'static str>) -> bool {
    lines.any(|line| wanted(line) && line.starts_with('#'))
}

// Bad: a prefix-shaped adapter, whose tests lift into a `take_while`
// rather than a `filter`.
fn take_while_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .take_while(|line| wanted(line) && line.starts_with('#'))
        .collect()
}

// Not flagged: one test is already one adapter asking one question.
fn one_test(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| wanted(line)).collect()
}

// Not flagged: a disjunction keeps items the pair would have dropped.
fn disjunction(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) || line.starts_with('#'))
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

// Not flagged: a conjunct naming no item holds for every item or none,
// so it wants hoisting out of the pipeline rather than an adapter.
fn invariant_conjunct(flag: bool, lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| flag && wanted(line)).collect()
}

// Not flagged: a conjunction of comparisons is one test. Each of these
// asks a single question that its halves do not: whether a byte is
// whitespace, and whether a position is inside a range.
fn whitespace_byte(bytes: std::slice::Iter<'static, u8>) -> bool {
    bytes.clone().any(|byte| *byte != b' ' && *byte != b'\t')
}

fn inside_a_range(positions: std::vec::IntoIter<usize>, start: usize, end: usize) -> bool {
    positions.into_iter().any(|pos| pos >= start && pos < end)
}

// Not flagged: one comparison among the conjuncts is enough, since the
// rule cannot tell which bound belongs to which question.
fn a_comparison_among_calls(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| line.len() > 3 && wanted(line)).collect()
}

// Not flagged: a destructured parameter, where each test would have to
// reproduce the pattern.
fn destructured(pairs: std::vec::IntoIter<(usize, &'static str)>) -> Vec<(usize, &'static str)> {
    pairs
        .filter(|(key, value)| *key > 0 && wanted(value))
        .collect()
}

// Not flagged: `Option::filter` is a method of the same name whose item
// the rule does not speak about.
fn option_filter(line: Option<&'static str>) -> Option<&'static str> {
    line.filter(|line| wanted(line) && line.starts_with('#'))
}

fn main() {}
