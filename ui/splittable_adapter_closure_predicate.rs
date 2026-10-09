// edition:2024
//
// What counts as one test, and how many a predicate runs. The `&&` the
// reader wrote is what the trigger reads, so a disjunction is one test
// however many comparisons it holds, a conjunct naming no item tests
// nothing about the item, and a `&&` an expansion wrote is not one anybody
// can cut.
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

// Bad: two tests on the item.
fn two_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) && line.starts_with('#'))
        .collect()
}

// Good: one adapter per test, which is what the rule asks for.
fn split_two_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
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

// Good: one adapter per test, however many there are.
fn split_three_tests(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line))
        .filter(|line| line.starts_with('#'))
        .filter(|line| line.ends_with('!'))
        .collect()
}

// Bad: a disjunction inside a conjunct stays whole, and the pair still
// splits.
fn conjunct_holds_a_disjunction(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) && (line.starts_with('#') || line.ends_with('!')))
        .collect()
}

// Good: the disjunction stays one test, in an adapter of its own.
fn split_conjunct_holds_a_disjunction(
    lines: std::vec::IntoIter<&'static str>,
) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line))
        .filter(|line| line.starts_with('#') || line.ends_with('!'))
        .collect()
}

// Bad: a comment inside the predicate does not stop it being two tests, so
// the rule still asks for the split. It offers the rewrite as advice rather
// than applying it, splicing the adapters being what would drop the comment.
fn a_commented_conjunction(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line) /* and a heading */ && line.starts_with('#'))
        .collect()
}

// Good: one adapter per test, with the comment kept beside the test it was
// about.
fn split_a_commented_conjunction(
    lines: std::vec::IntoIter<&'static str>,
) -> Vec<&'static str> {
    lines
        .filter(|line| wanted(line))
        .filter(|line| line.starts_with('#') /* a heading */)
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

// Not flagged: a conjunct naming no item holds for every item or none,
// so it wants hoisting out of the pipeline rather than an adapter.
fn invariant_conjunct(flag: bool, lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| flag && wanted(line)).collect()
}

// Not flagged: a destructured parameter, where each test would have to
// reproduce the pattern.
fn destructured(pairs: std::vec::IntoIter<(usize, &'static str)>) -> Vec<(usize, &'static str)> {
    pairs
        .filter(|(key, value)| *key > 0 && wanted(value))
        .collect()
}

macro_rules! both {
    ($line:expr) => {
        wanted($line) && $line.starts_with('#')
    };
}

// Not flagged: a conjunction the reader did not write has no `&&` they
// can cut.
fn conjunction_from_a_macro(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines.filter(|line| both!(line)).collect()
}

fn main() {}
