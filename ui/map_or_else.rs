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

macro_rules! defaulting {
    ($option:expr) => {
        $option.map_or_else(|| false, |value| value > 1)
    };
}

fn option_default(width: Option<u32>) -> u32 {
    // Bad: the default closure is read before the mapper it stands in
    // for.
    width.map_or_else(|| 80, |width| width.saturating_sub(2))
}

fn result_default(parsed: Result<u32, String>) -> u32 {
    // Bad: the closure taking the error comes first, so the mapper
    // reads last.
    parsed.map_or_else(|error| error.len() as u32, |width| width + 1)
}

fn constructed_default(name: Option<&str>) -> String {
    // Bad: `String::new` as the default means the chain can end in
    // `unwrap_or_default`.
    name.map_or_else(String::new, |name| name.to_uppercase())
}

fn branching_default(parsed: Result<u32, String>) -> u32 {
    // Bad: a default closure holding a branch of its own is where the
    // `match` remedy reads better than the chain.
    parsed.map_or_else(
        |error| if error.is_empty() { 0 } else { 1 },
        |width| width + 1,
    )
}

fn through_a_reference(width: &Option<u32>) -> u32 {
    // Bad: a `&Option<_>` receiver resolves `map_or_else` through a
    // deref, since the payload is `Copy`.
    width.map_or_else(|| 0, |width| width + 1)
}

fn inside_a_macro_argument(width: Option<u32>) {
    // Bad: a call written as a macro argument carries the author's own
    // span, so it is theirs to fix.
    assert!(width.map_or_else(|| false, |width| width > 2));
}

fn chained(parsed: Result<u32, String>) -> u32 {
    // Good: the chain states the receiver, then the mapper, then what
    // stands in when there is nothing to map.
    parsed
        .map(|width| width + 1)
        .unwrap_or_else(|error| error.len() as u32)
}

fn defaulted(name: Option<&str>) -> String {
    // Good: the chain's shorter ending, where the default closure only
    // built the type's own `Default`.
    name.map(|name| name.to_uppercase()).unwrap_or_default()
}

fn matched(parsed: Result<u32, String>) -> u32 {
    // Good: a `match` names both cases and keeps the mapper next to the
    // value it maps.
    match parsed {
        Ok(width) => width + 1,
        Err(error) if error.is_empty() => 0,
        Err(_) => 1,
    }
}

fn defaulted_mapper_only(name: Option<&str>) -> usize {
    // Not flagged: `map_or_default` takes the mapper and nothing else,
    // so it has no order to get wrong.
    name.map_or_default(|name| name.len())
}

fn another_receiver(slot: Slot) -> u32 {
    // Not flagged: the receiver is neither an `Option` nor a `Result`,
    // whatever its own `map_or_else` is shaped like.
    slot.map_or_else(|| 0, |value| value + 1)
}

fn from_an_expansion(width: Option<u32>) -> bool {
    // Not flagged: the call belongs to the `defaulting!` definition
    // rather than to this line.
    defaulting!(width)
}

fn main() {}
