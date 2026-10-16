// edition:2024
//
// What the names in the closure resolve to. `and_then`, `map`, `filter`,
// `then` and `then_some` are the combinators the trigger reads, and each
// is also an ordinary method name: a type of one's own may declare any of
// them, and a trait of one's own may declare the `filter_map` the call
// sits under. The receiver's type is what tells the combinator from the
// method, and the adapter's trait what tells the iterator's `filter_map`
// from another.
//
// A call that is a method of that name is still a step, so a Bad case
// here is followed by the Good form the chain trigger asks for.

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

fn wanted(line: &str) -> bool {
    !line.is_empty()
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

// Bad: `and_then` on something that is not an `Option` is a method of
// that name rather than the combinator, so the guard-and-value trigger has
// no split for it. The call is a step all the same.
fn method_of_that_name(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| sieve(line).and_then(parse)).collect()
}

// Good: the chain's split, the call of that name staying where it is.
fn split_method_of_that_name(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines
        .map(sieve)
        .filter_map(|held| held.and_then(parse))
        .collect()
}

// Not flagged: `then_some` on something that is not a `bool` is out of
// scope, the guard the trigger reads being the one a `bool` answers.
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

// Bad: a `map` whose receiver is the `Option` the step before it made is
// an ordinary step, named like the combinator seam without being it.
fn map_of_that_name(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.filter_map(|line| sieve(line).map(render)).collect()
}

// Good: the step rooted at the item lifted, leaving the adapter the `map`
// that was never its own.
fn split_map_of_that_name(lines: std::vec::IntoIter<&'static str>) -> Vec<usize> {
    lines.map(sieve).filter_map(|held| held.map(render)).collect()
}

// Bad: a `filter` whose receiver is the `Option` the step before it made is
// an ordinary step, the name saying nothing about what it is called on.
fn filter_of_that_name(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .filter_map(|line| sieve(line).filter(|text| wanted(text)))
        .collect()
}

// Good: the step rooted at the item lifted, leaving the adapter the
// `filter` it was handed.
fn split_filter_of_that_name(lines: std::vec::IntoIter<&'static str>) -> Vec<&'static str> {
    lines
        .map(sieve)
        .filter_map(|held| held.filter(|text| wanted(text)))
        .collect()
}

fn main() {}
