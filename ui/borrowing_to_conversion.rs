// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::path::{Path, PathBuf};

struct Person {
    name: String,
    home: PathBuf,
    tags: Vec<String>,
    nickname: Option<String>,
    age: u32,
}

impl Person {
    // Bad: `to_` promises an owned value, this borrows.
    fn to_name(&self) -> &str {
        &self.name
    }

    // Bad: same, returning `&Path`.
    fn to_home(&self) -> &Path {
        &self.home
    }

    // Bad: same, returning a slice.
    fn to_tags(&self) -> &[String] {
        &self.tags
    }

    // Bad: an `Option` of a borrow is as free as the borrow inside it.
    fn to_nickname(&self) -> Option<&str> {
        self.nickname.as_deref()
    }

    // Good: `to_` returning an owned value is what the prefix promises.
    fn to_owned_name(&self) -> String {
        self.name.clone()
    }

    // Good: an owned `Option` is not a borrow.
    fn to_nickname_owned(&self) -> Option<String> {
        self.nickname.clone()
    }

    // Good: a `Copy` value is owned, not borrowed.
    fn to_age(&self) -> u32 {
        self.age
    }

    // Good: `as_*` is the right prefix for a free conversion.
    fn as_name(&self) -> &str {
        &self.name
    }

    // Good: not the `to_` prefix.
    fn token(&self) -> &str {
        &self.name
    }

    // Good: takes an argument, so it is not a conversion of `self`.
    fn to_name_or(&self, fallback: &str) -> &str {
        &self.name
    }
}

// Good: a trait fixes the signature.
trait ToName {
    fn to_name(&self) -> &str;
}

impl ToName for Person {
    fn to_name(&self) -> &str {
        &self.name
    }
}

fn main() {}
