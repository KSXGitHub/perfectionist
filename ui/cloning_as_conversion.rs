// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::path::{Path, PathBuf};

struct Person {
    name: String,
    home: PathBuf,
    tags: Vec<String>,
    age: u32,
    label: Box<str>,
}

impl Person {
    // Bad: `as_` promises free, this allocates.
    fn as_name(&self) -> String {
        self.name.clone()
    }

    // Bad: same, through `to_path_buf`.
    fn as_home(&self) -> PathBuf {
        self.home.to_path_buf()
    }

    // Bad: same, through `to_vec`.
    fn as_tags(&self) -> Vec<String> {
        self.tags.to_vec()
    }

    // Good: the borrowed forms cost nothing, which is what `as_` says.
    fn as_name_ref(&self) -> &str {
        &self.name
    }

    fn as_home_ref(&self) -> &Path {
        &self.home
    }

    // Good: a `Copy` field returned by value is free.
    fn as_age(&self) -> u32 {
        self.age
    }

    // Good: cloning a `Copy` field still returns it by value.
    fn as_cloned_age(&self) -> u32 {
        self.age.clone()
    }

    // Not flagged: takes `self` by value, so the receiver excludes it
    // before the body is read. An `as_*` that consumes is its own
    // mistake, but not this rule's.
    fn as_owned_name(self) -> String {
        self.name
    }

    // Not flagged: not the `as_` prefix. `to_*` announces a conversion
    // that costs something, so the copy is what that name promises.
    fn to_name(&self) -> String {
        self.name.clone()
    }

    // Not flagged: not the `as_` prefix, though `as` without the
    // underscore is close enough to pin the boundary.
    fn ascii_name(&self) -> String {
        self.name.clone()
    }

    // Not flagged: takes an argument, so it is not a conversion of
    // `self`. The body is the shape this rule fires on, so this pins
    // that the arity requirement is what excludes it.
    fn as_name_or(&self, fallback: &str) -> String {
        self.name.clone()
    }

    // Not flagged: `&mut self` is not the receiver this measures.
    fn as_taken_name(&mut self) -> String {
        self.name.clone()
    }
}

// Not flagged: a trait fixes the signature, so the impl cannot change it.
trait AsName {
    fn as_name(&self) -> String;
}

impl AsName for Person {
    fn as_name(&self) -> String {
        self.name.clone()
    }
}

fn main() {}
