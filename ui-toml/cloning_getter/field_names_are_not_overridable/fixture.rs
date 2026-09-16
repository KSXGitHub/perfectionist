// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Run with `getter_name_patterns = ["!*"]`, the strongest thing the
// list can say: no name it reaches is a getter. A method named for a
// field of `self` is one anyway, because the list never reaches it.

struct Person {
    first_name: String,
}

impl Person {
    // Bad: named for a field, which no entry talks the rule out of.
    fn first_name(&self) -> String {
        self.first_name.clone()
    }

    // Not flagged: `!*` covers every name the list does reach, and
    // `get_first_name` names no field.
    fn get_first_name(&self) -> String {
        self.first_name.clone()
    }

    fn unrelated(&self) -> String {
        self.first_name.clone()
    }
}

fn main() {}
