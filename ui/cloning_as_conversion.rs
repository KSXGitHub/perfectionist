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

    // Good: moving a field out is free — nothing is copied.
    fn as_owned_name(self) -> String {
        self.name
    }

    // Good: `to_*` is the prefix for a conversion that costs something,
    // so the copy is what the name already promises.
    fn to_name(&self) -> String {
        self.name.clone()
    }

    // Good: not the `as_` prefix — `cloning_getter` measures this one.
    fn ascii_name(&self) -> String {
        self.name.clone()
    }

    // Good: takes an argument, so it is not a conversion of `self`.
    fn as_name_or(&self, fallback: &str) -> String {
        self.name.clone()
    }

    // Good: `&mut self` is not the `as_` receiver shape this measures.
    fn as_taken_name(&mut self) -> String {
        self.name.clone()
    }
}

// Good: a trait fixes the signature.
trait AsName {
    fn as_name(&self) -> String;
}

impl AsName for Person {
    fn as_name(&self) -> String {
        self.name.clone()
    }
}

fn main() {}
