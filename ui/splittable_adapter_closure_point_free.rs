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

use std::path::{Path, PathBuf};

/// Takes a `&Path`, which a `&PathBuf` reaches only through a deref.
fn canonical(path: &Path) -> Result<PathBuf, ()> {
    match path.is_absolute() {
        true => Ok(path.to_path_buf()),
        false => Err(()),
    }
}

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

// Bad: the first step is a call whose argument reaches the parameter through
// a `Deref` impl, `&PathBuf` to `&Path`. Naming the function would hand it a
// `&PathBuf`, which it does not take, so the step keeps a closure where a
// step handed exactly what it takes would not.
fn coerced(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.iter().filter_map(|file| canonical(file).ok()).collect()
}

// Good: one adapter per step, the coercing call keeping its closure and
// `ok` written as the path it can be.
fn split_coerced(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|file| canonical(file))
        .filter_map(Result::ok)
        .collect()
}

fn main() {}
