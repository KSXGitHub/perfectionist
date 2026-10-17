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

fn literal_default(width: Option<u32>) -> u32 {
    // Bad: the default `80` is read before the mapper that explains it.
    width.map_or(80, |width| width.saturating_sub(2))
}

fn zero_default(width: Option<u32>) -> u32 {
    // Bad: a literal `0` names the value it stands for, so the help
    // offers the chain rather than `unwrap_or_default`.
    width.map_or(0, |width| width + 1)
}

fn option_false_default(width: Option<u32>) -> bool {
    // Bad: defaulting an `Option` to `false` is what `is_some_and` means.
    width.map_or(false, |width| width > 2)
}

fn option_true_default(width: Option<u32>) -> bool {
    // Bad: defaulting an `Option` to `true` is what `is_none_or` means.
    width.map_or(true, |width| width > 2)
}

fn result_false_default(width: Result<u32, String>) -> bool {
    // Bad: defaulting a `Result` to `false` is what `is_ok_and` means.
    width.map_or(false, |width| width > 2)
}

fn result_true_default(width: Result<u32, String>) -> bool {
    // Bad: `Result` has no counterpart of `is_none_or`, so a `true`
    // default takes the chain.
    width.map_or(true, |width| width > 2)
}

fn option_none_default(width: Option<u32>) -> Option<u32> {
    // Bad: defaulting an `Option` to `None` is what `and_then` means.
    width.map_or(None, |width| width.checked_add(1))
}

fn result_none_default(width: Result<u32, String>) -> Option<u32> {
    // Bad: reaching `and_then` from a `Result` would take an `ok()`
    // first, so this default takes the chain.
    width.map_or(None, |width| width.checked_add(1))
}

fn constructed_default(name: Option<&str>) -> String {
    // Bad: `String::new()` is the default `String`, so the chain can
    // end in `unwrap_or_default`.
    name.map_or(String::new(), |name| name.to_uppercase())
}

fn trait_default(width: Option<u32>) -> u32 {
    // Bad: `Default::default()` says outright that the chain can end in
    // `unwrap_or_default`.
    width.map_or(Default::default(), |width| width + 1)
}

fn through_a_reference(width: &Option<u32>) -> u32 {
    // Bad: a `&Option<_>` receiver resolves `map_or` through a deref,
    // since the payload is `Copy`.
    width.map_or(0, |width| width + 1)
}

fn inside_a_macro_argument(width: Option<u32>) {
    // Bad: a call written as a macro argument carries the author's own
    // span, so it is theirs to fix.
    assert!(width.map_or(false, |width| width > 2));
}

fn chained(width: Option<u32>) -> u32 {
    // Good: the chain states the receiver, then the mapper, then what
    // stands in when there is nothing to map.
    width.map(|width| width.saturating_sub(2)).unwrap_or(80)
}

fn tested(width: Option<u32>) -> bool {
    // Good: the predicate says what a `false` default was standing in
    // for.
    width.is_some_and(|width| width > 2)
}

fn matched(width: Option<u32>) -> u32 {
    // Good: a `match` names both cases and keeps the default off the
    // path that has a value.
    match width {
        Some(width) => width.saturating_sub(2),
        None => 80,
    }
}

fn defaulted_mapper_only(name: Option<&str>) -> usize {
    // Not flagged: `map_or_default` takes the mapper and nothing else,
    // so it has no order to get wrong.
    name.map_or_default(|name| name.len())
}

fn another_receiver(slot: Slot) -> u32 {
    // Not flagged: the receiver is neither an `Option` nor a `Result`,
    // whatever its own `map_or` is shaped like.
    slot.map_or(0, |value| value + 1)
}

fn from_an_expansion(width: Option<u32>) -> bool {
    // Not flagged: the call belongs to the `defaulting!` definition
    // rather than to this line.
    defaulting!(width)
}

fn main() {}
