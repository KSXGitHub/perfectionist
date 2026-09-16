// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Run with `getter_name_patterns = ["!*", "get_*"]`. The leading `!*`
// matches every name, so nothing is left for the field match to
// decide; the `get_*` after it takes one shape back. This is how a
// project measures its `get_*` methods and nothing else.

struct Person {
    first_name: String,
}

impl Person {
    // Bad: the later `get_*` overrides the `!*`.
    fn get_first_name(&self) -> String {
        self.first_name.clone()
    }

    // Not flagged: `!*` matches it, and nothing after that does, so the
    // field match never runs.
    fn first_name(&self) -> String {
        self.first_name.clone()
    }

    // Not flagged: `!*` matches this too.
    fn unrelated(&self) -> String {
        self.first_name.clone()
    }
}

fn main() {}
