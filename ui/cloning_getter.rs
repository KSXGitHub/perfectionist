// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

use std::path::{Path, PathBuf};

struct Person {
    first_name: String,
    middle_name: Option<String>,
    home: PathBuf,
    tags: Vec<String>,
    age: u32,
}

impl Person {
    // Bad: `clone` of a `String` field.
    fn first_name(&self) -> String {
        self.first_name.clone()
    }

    // Bad: `clone` of an `Option<String>` field.
    fn middle_name(&self) -> Option<String> {
        self.middle_name.clone()
    }

    // Bad: `to_path_buf` after a deref is still the field copied out.
    fn home(&self) -> PathBuf {
        self.home.to_path_buf()
    }

    // Bad: `to_vec`.
    fn tags(&self) -> Vec<String> {
        self.tags.to_vec()
    }

    // Bad: `to_string` on a `String` field.
    fn display_name(&self) -> String {
        self.first_name.to_string()
    }

    // Good: the borrowed forms.
    fn first_name_ref(&self) -> &str {
        &self.first_name
    }

    fn middle_name_ref(&self) -> Option<&str> {
        self.middle_name.as_deref()
    }

    fn home_ref(&self) -> &Path {
        &self.home
    }

    // Good: a `Copy` field returned by value is not a clone.
    fn age(&self) -> u32 {
        self.age
    }

    // Good: not a getter — the body does more than copy a field.
    fn shouted(&self) -> String {
        self.first_name.to_uppercase()
    }

    // Good: not a getter — it takes an argument.
    fn name_or(&self, fallback: &str) -> String {
        if self.first_name.is_empty() { fallback.to_owned() } else { self.first_name.clone() }
    }

    // Good: `&mut self` is a mutator, not a getter.
    fn take_name(&mut self) -> String {
        self.first_name.clone()
    }
}

// Good: a trait fixes the signature.
trait Named {
    fn name(&self) -> String;
}

impl Named for Person {
    fn name(&self) -> String {
        self.first_name.clone()
    }
}

// Good: a method a macro expands to is not measured.
macro_rules! getter {
    ($name:ident, $field:ident) => {
        impl Person {
            fn $name(&self) -> String {
                self.$field.clone()
            }
        }
    };
}

getter!(generated_name, first_name);

fn main() {}
