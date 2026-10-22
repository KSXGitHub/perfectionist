// edition:2024
#![allow(dead_code, unused, reason = "ui fixture")]

// `exempt_simple_default = false` takes the exemption out, so a path
// default is flagged like the closures are. A closure is flagged
// whatever this knob says, so every case here names its default.

/// A type whose `new` hands back something other than its `Default`,
/// which is the shape a fix keyed on the name `new` alone would get
/// wrong.
struct Port(u16);

impl Port {
    fn new() -> Self {
        Port(8080)
    }
}

impl Default for Port {
    fn default() -> Self {
        Port(0)
    }
}

/// A fallback this fixture names rather than spells out.
fn spare() -> u32 {
    7
}

// Bad: `String::new` is what `String`'s `Default` gives, which the fix
// takes.
fn constructed_default(name: Option<&str>) -> String {
    name.map_or_else(String::new, |name| name.to_uppercase())
}

// Bad, and no fix: `Port::new` hands back `Port(8080)` where
// `Port::default` hands back `Port(0)`, so the call keeps its default
// and the help names the open remedies.
fn constructor_that_is_not_the_default(width: Option<u16>) -> Port {
    width.map_or_else(Port::new, Port)
}

// Bad: a function named on its own is a path, and the exemption was
// the only thing keeping it quiet.
fn identifier_default(width: Option<u32>) -> u32 {
    width.map_or_else(spare, |width| width + 1)
}

fn main() {}
