// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Run with `getter_name_patterns = ["*", "!clone_*"]`. The `*` calls
// every name a getter, and the `!clone_*` after it takes the `clone_*`
// ones back out, so this fixture is where a later entry overriding an
// earlier one is visible.

struct Person {
    first_name: String,
}

impl Person {
    // Bad: `*` admits a name the field match would have missed.
    fn unrelated(&self) -> String {
        self.first_name.clone()
    }

    // Bad: `cloned_*` is not `clone_*`, so the negation leaves it to
    // the `*` before it.
    fn cloned_first_name(&self) -> String {
        self.first_name.clone()
    }

    // Not flagged: the later `!clone_*` overrides the `*`.
    fn clone_first_name(&self) -> String {
        self.first_name.clone()
    }

    // Bad: named for a field, so the rule decides it before the list
    // is consulted at all.
    fn first_name(&self) -> String {
        self.first_name.clone()
    }

    // Not flagged: the conversion clause decides the name above the
    // list, so no entry reaches it.
    fn to_first_name(&self) -> String {
        self.first_name.clone()
    }
}

fn main() {}
