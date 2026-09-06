// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// With `max_lines = 0` every function with at least one line of code is
// flagged and the diagnostic states the count, which pins what counts
// as a line.

fn work(value: u32) -> u32 {
    value
}

// Not flagged: an empty body has no lines.
fn empty() {}

// Not flagged: a body of only a comment has no lines of code.
fn only_a_comment() {
    // nothing to do
}

// 1: the braces are not counted.
fn one_line() {
    work(1);
}

// 2: a blank line between two statements is not counted.
fn two_lines_with_a_gap() {
    work(1);

    work(2);
}

// 3: a string literal counts every line it spans.
fn multi_line_string() -> &'static str {
    "one
two
three"
}

// 1: a trailing comment does not make a second line.
fn trailing_comment() {
    work(1); // done
}

// 1: a block comment spanning lines is not counted.
fn block_comment() {
    /* one
       two */
    work(1);
}

// 4: a nested function's lines belong to the body that holds it.
fn with_nested_function() {
    fn inner() {
        work(1);
    }
    inner();
}

// 1: an expression body counts its own line.
fn single_expression() -> u32 {
    work(1)
}

// 2: a method body is counted like a free function's.
struct Counter;

impl Counter {
    fn count(&self) -> u32 {
        let value = work(1);
        value
    }
}

// 1: a macro invocation counts the lines its call spans, not the lines
// of its expansion.
fn formatting() -> String {
    format!("{}", work(1))
}

fn main() {}
