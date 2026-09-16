// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Run with `measure_unmatched_names = true`, so clause 4 of the getter
// definition admits a method whose name neither starts with `get_` nor
// names a field. Clauses 1 to 3 still take precedence over it.

struct Person {
    first_name: String,
}

impl Person {
    // Clause 4: admitted only because the knob is on.
    fn unrelated(&self) -> String {
        self.first_name.clone()
    }

    fn cloned_first_name(&self) -> String {
        self.first_name.clone()
    }

    // Clause 3: names a field, so it is a getter either way.
    fn first_name(&self) -> String {
        self.first_name.clone()
    }

    // Clause 2: `get_*` is a getter either way.
    fn get_anything(&self) -> String {
        self.first_name.clone()
    }

    // Clause 1 overrides the knob: these are conversions, never getters.
    fn to_first_name(&self) -> String {
        self.first_name.clone()
    }

    fn into_first_name(&self) -> String {
        self.first_name.clone()
    }

    fn as_first_name(&self) -> String {
        self.first_name.clone()
    }
}

fn main() {}
