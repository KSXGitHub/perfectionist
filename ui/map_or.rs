// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

/// A type of this fixture's own, carrying a method named `map_or` that
/// is shaped like the std one without being it.
struct Slot(u32);

impl Slot {
    fn map_or<Mapped>(self, default: Mapped, mapper: impl FnOnce(u32) -> Mapped) -> Mapped {
        let _ = default;
        mapper(self.0)
    }
}

macro_rules! defaulting {
    ($option:expr) => {
        $option.map_or(false, |value| value > 1)
    };
}

// Bad: the default `80` is read before the mapper that explains it.
fn literal_default(width: Option<u32>) -> u32 {
    width.map_or(80, |width| width.saturating_sub(2))
}

// Bad: a literal `0` names the value it stands for, so the help
// offers the chain rather than `unwrap_or_default`.
fn zero_default(width: Option<u32>) -> u32 {
    width.map_or(0, |width| width + 1)
}

// Bad: defaulting an `Option` to `false` is what `is_some_and` means.
fn option_false_default(width: Option<u32>) -> bool {
    width.map_or(false, |width| width > 2)
}

// Bad: defaulting an `Option` to `true` is what `is_none_or` means.
fn option_true_default(width: Option<u32>) -> bool {
    width.map_or(true, |width| width > 2)
}

// Bad: defaulting a `Result` to `false` is what `is_ok_and` means.
fn result_false_default(width: Result<u32, String>) -> bool {
    width.map_or(false, |width| width > 2)
}

// Bad: `Result` has no counterpart of `is_none_or`, so a `true`
// default takes the chain.
fn result_true_default(width: Result<u32, String>) -> bool {
    width.map_or(true, |width| width > 2)
}

// Bad: defaulting an `Option` to `None` is what `and_then` means.
fn option_none_default(width: Option<u32>) -> Option<u32> {
    width.map_or(None, |width| width.checked_add(1))
}

// Bad: reaching `and_then` from a `Result` would take an `ok()`
// first, so this default takes the chain.
fn result_none_default(width: Result<u32, String>) -> Option<u32> {
    width.map_or(None, |width| width.checked_add(1))
}

// Bad: `String::new()` is the default `String`, so the chain can
// end in `unwrap_or_default`.
fn constructed_default(name: Option<&str>) -> String {
    name.map_or(String::new(), |name| name.to_uppercase())
}

// Bad: `Default::default()` says outright that the chain can end in
// `unwrap_or_default`.
fn trait_default(width: Option<u32>) -> u32 {
    width.map_or(Default::default(), |width| width + 1)
}

// Bad: a `&Option<_>` receiver resolves `map_or` through a deref,
// since the payload is `Copy`.
fn through_a_reference(width: &Option<u32>) -> u32 {
    width.map_or(0, |width| width + 1)
}

// Bad: a call written as a macro argument carries the author's own
// span, so it is theirs to fix.
fn inside_a_macro_argument(width: Option<u32>) {
    assert!(width.map_or(false, |width| width > 2));
}

// Good: the chain states the receiver, then the mapper, then what
// stands in when there is nothing to map.
fn chained(width: Option<u32>) -> u32 {
    width.map(|width| width.saturating_sub(2)).unwrap_or(80)
}

// Good: the predicate says what a `false` default was standing in
// for.
fn tested(width: Option<u32>) -> bool {
    width.is_some_and(|width| width > 2)
}

// Good: a `match` names both cases and keeps the default off the
// path that has a value.
fn matched(width: Option<u32>) -> u32 {
    match width {
        Some(width) => width.saturating_sub(2),
        None => 80,
    }
}

// Not flagged: `map_or_default` takes the mapper and nothing else,
// so it has no order to get wrong.
fn defaulted_mapper_only(name: Option<&str>) -> usize {
    name.map_or_default(|name| name.len())
}

// Not flagged: the receiver is neither an `Option` nor a `Result`,
// whatever its own `map_or` is shaped like.
fn another_receiver(slot: Slot) -> u32 {
    slot.map_or(0, |value| value + 1)
}

// Not flagged: the call belongs to the `defaulting!` definition
// rather than to this line.
fn from_an_expansion(width: Option<u32>) -> bool {
    defaulting!(width)
}

fn main() {}
