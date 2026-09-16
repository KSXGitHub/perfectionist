// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Run with `measure_unmatched_names = true` so clause 5 is open, plus
// `ignore_exempt_prefixes = ["cloned_"]` and
// `extra_exempt_prefixes = ["copy_"]`. Clause 2 is what those two knobs
// move, and it still overrides clause 5.

struct Person {
    first_name: String,
}

impl Person {
    // Bad: `cloned_` was dropped from the roster, so clause 2 no longer
    // covers it and clause 5 admits it.
    fn cloned_first_name(&self) -> String {
        self.first_name.clone()
    }

    // Not flagged: `clone_` is still on the roster.
    fn clone_first_name(&self) -> String {
        self.first_name.clone()
    }

    // Not flagged: `copy_` was added to the roster, so clause 2 covers
    // it even though clause 5 is open.
    fn copy_first_name(&self) -> String {
        self.first_name.clone()
    }

    // Bad: clause 5 admits a name the roster does not cover.
    fn unrelated(&self) -> String {
        self.first_name.clone()
    }

    // Not flagged: clause 1 overrides the roster and clause 5 alike.
    fn to_first_name(&self) -> String {
        self.first_name.clone()
    }
}

fn main() {}
