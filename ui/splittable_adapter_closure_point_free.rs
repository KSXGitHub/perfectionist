// edition:2024
//
// How a lifted step is written once it has an adapter of its own. A step
// keeps its own name and takes the path of the type it was applied to, so
// the type a step was handed is what decides the path, and a primitive
// answers under its own name where a struct answers under the struct's.
//
// Every Bad case is followed by the Good one the rule's help asks for. A
// form the rule suggests and then fires on again is a false positive that
// reading the Bad cases alone would never find, and a form that does not
// compile is advice nobody can take.

#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Bad: the step is applied to a `usize`, so the path is written under
// `usize`.
fn unsigned(counts: Vec<usize>) -> Vec<usize> {
    counts.into_iter().map(|count| count.to_string().len()).collect()
}

// Good: one adapter per step. Neither step takes what it is applied to by
// value, so each keeps a closure rather than a path.
fn split_unsigned(counts: Vec<usize>) -> Vec<usize> {
    counts
        .into_iter()
        .map(|count| count.to_string())
        .map(|text| text.len())
        .collect()
}

// Bad: the step is applied to an `i32`, whose path is written under the
// signed name rather than the unsigned one.
fn signed(offsets: Vec<i32>) -> Vec<String> {
    offsets.into_iter().map(|offset| offset.abs().to_string()).collect()
}

// Good: one adapter per step, `abs` under the path it takes by value and
// `to_string` keeping a closure because it borrows.
fn split_signed(offsets: Vec<i32>) -> Vec<String> {
    offsets
        .into_iter()
        .map(i32::abs)
        .map(|offset| offset.to_string())
        .collect()
}

// Bad: the step is applied to an `f64`.
fn floating(sizes: Vec<f64>) -> Vec<String> {
    sizes.into_iter().map(|size| size.abs().to_string()).collect()
}

// Good: one adapter per step.
fn split_floating(sizes: Vec<f64>) -> Vec<String> {
    sizes.into_iter().map(f64::abs).map(|size| size.to_string()).collect()
}

// Bad: the step is applied to a `bool`.
fn flags(flags: Vec<bool>) -> Vec<usize> {
    flags.into_iter().map(|flag| flag.to_string().len()).collect()
}

// Good: one adapter per step.
fn split_flags(flags: Vec<bool>) -> Vec<usize> {
    flags
        .into_iter()
        .map(|flag| flag.to_string())
        .map(|text| text.len())
        .collect()
}

// Bad: the step is applied to a `char`.
fn letters(letters: Vec<char>) -> Vec<usize> {
    letters.into_iter().map(|letter| letter.to_string().len()).collect()
}

// Good: one adapter per step.
fn split_letters(letters: Vec<char>) -> Vec<usize> {
    letters
        .into_iter()
        .map(|letter| letter.to_string())
        .map(|text| text.len())
        .collect()
}

fn main() {}
