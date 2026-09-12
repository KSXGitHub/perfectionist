// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

// Bad: the set that does the deduplicating has no name.
fn deduplicate(names: Vec<String>) -> Vec<String> {
    names.into_iter().collect::<HashSet<_>>().into_iter().collect()
}

// Bad: the sorted set is built, walked, and rebuilt as a vector.
fn sorted(values: Vec<u32>) -> Vec<u32> {
    values.into_iter().collect::<BTreeSet<_>>().into_iter().rev().collect()
}

// Good: a container that merely arrives is converted one way.
fn convert(paths: Vec<String>) -> Vec<PathBuf> {
    paths.into_iter().map(PathBuf::from).collect()
}

// Good: the walk ends at its first collection.
fn tally(values: &[u32]) -> HashMap<u32, usize> {
    values.iter().map(|value| (*value, 1)).collect()
}

// Good: collecting, then asking the collection something, is one
// computation reported by another rule if the chain grows too long.
fn longest(names: &[String]) -> usize {
    names.iter().map(String::len).max().unwrap_or_default()
}

// Good: the second collection comes from a fresh source, not from a
// walk of the first.
fn pair(names: Vec<String>, values: Vec<u32>) -> (HashSet<String>, Vec<u32>) {
    let unique: HashSet<String> = names.into_iter().collect();
    (unique, values.into_iter().rev().collect())
}

fn main() {}
