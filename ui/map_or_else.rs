// run-rustfix
// rustfix-only-machine-applicable
// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

/// A type of this fixture's own, carrying a method named `map_or_else`
/// that is shaped like the std one without being it.
struct Slot(u32);

impl Slot {
    fn map_or_else<Mapped>(
        self,
        default: impl FnOnce() -> Mapped,
        mapper: impl FnOnce(u32) -> Mapped,
    ) -> Mapped {
        let _ = default;
        mapper(self.0)
    }
}

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

macro_rules! defaulting {
    ($option:expr) => {
        $option.map_or_else(|| false, |value| value > 1)
    };
}

// Bad: the default closure is read before the mapper it stands in
// for.
fn option_default(width: Option<u32>) -> u32 {
    width.map_or_else(|| 80, |width| width.saturating_sub(2))
}

// Bad: the closure taking the error comes first, so the mapper
// reads last.
fn result_default(parsed: Result<u32, String>) -> u32 {
    parsed.map_or_else(|error| error.len() as u32, |width| width + 1)
}

// Bad: a closure handing back the type's `Default` is what the fix
// takes.
fn closure_default(width: Option<u32>) -> u32 {
    width.map_or_else(|| 0, |width| width + 1)
}

// Bad: a `Result` closure that ignores the error and hands back the
// type's `Default` qualifies for the fix, since `map_or_default`
// ignores the error too.
fn error_ignored_default(name: Result<&str, String>) -> String {
    name.map_or_else(|_| String::new(), |name| name.to_uppercase())
}

// Bad: a closure handing back `None` on an `Option` is what `and_then`
// means, which wins over `None` being an `Option`'s own `Default`.
fn option_none_default(width: Option<u32>) -> Option<u32> {
    width.map_or_else(|| None, |width| width.checked_add(1))
}

// Bad: a default closure holding a branch of its own is where the
// `match` remedy reads better than the chain.
fn branching_default(parsed: Result<u32, String>) -> u32 {
    parsed.map_or_else(
        |error| if error.is_empty() { 0 } else { 1 },
        |width| width + 1,
    )
}

// Bad: a `&Option<_>` receiver resolves `map_or_else` through a
// deref, since the payload is `Copy`.
fn through_a_reference(width: &Option<u32>) -> u32 {
    width.map_or_else(|| 0, |width| width + 1)
}

// Bad: a call written as a macro argument carries the author's own
// span, so it is theirs to fix.
fn inside_a_macro_argument(width: Option<u32>) {
    assert!(width.map_or_else(|| false, |width| width > 2));
}

// Good: the chain states the receiver, then the mapper, then what
// stands in when there is nothing to map.
fn chained(parsed: Result<u32, String>) -> u32 {
    parsed
        .map(|width| width + 1)
        .unwrap_or_else(|error| error.len() as u32)
}

// Good: the `unwrap_or_default` ending, where the default closure
// only built the type's own `Default`.
fn defaulted(name: Option<&str>) -> String {
    name.map(|name| name.to_uppercase()).unwrap_or_default()
}

// Good: a `match` names both cases and keeps the mapper next to the
// value it maps.
fn matched(parsed: Result<u32, String>) -> u32 {
    match parsed {
        Ok(width) => width + 1,
        Err(error) if error.is_empty() => 0,
        Err(_) => 1,
    }
}

/// A fallback this fixture names rather than spells out, which is the
/// point-free default the exemption is for.
fn spare() -> u32 {
    7
}

// Not flagged: `String::new` is a path, and the reader holding a named
// default as far as the mapper costs nothing.
fn constructed_default(name: Option<&str>) -> String {
    name.map_or_else(String::new, |name| name.to_uppercase())
}

// Not flagged: `Port::new` is a path, whatever it hands back, so the
// exemption takes it before the fix ever weighs its body.
fn constructor_that_is_not_the_default(width: Option<u16>) -> Port {
    width.map_or_else(Port::new, Port)
}

// Not flagged: a function named on its own is a path, where the
// closure `|| spare()` spelling out the same call is not.
fn identifier_default(width: Option<u32>) -> u32 {
    width.map_or_else(spare, |width| width + 1)
}

// Not flagged: `map_or_default` takes the mapper and nothing else,
// so it has no order to get wrong.
fn defaulted_mapper_only(name: Option<&str>) -> usize {
    name.map_or_default(|name| name.len())
}

// Not flagged: the receiver is neither an `Option` nor a `Result`,
// whatever its own `map_or_else` is shaped like.
fn another_receiver(slot: Slot) -> u32 {
    slot.map_or_else(|| 0, |value| value + 1)
}

// Not flagged: the call belongs to the `defaulting!` definition
// rather than to this line.
fn from_an_expansion(width: Option<u32>) -> bool {
    defaulting!(width)
}

fn main() {}
