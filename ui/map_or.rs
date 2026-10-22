// edition:2024
// aux-build:serde_json_stub.rs
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

extern crate serde_json_stub as serde_json;

/// A type of this fixture's own, carrying a method named `map_or` that
/// is shaped like the std one without being it.
struct Slot(u32);

impl Slot {
    fn map_or<Mapped>(self, default: Mapped, mapper: impl FnOnce(u32) -> Mapped) -> Mapped {
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

/// A tuple one element wider than the twelve `Default` reaches, so
/// every element has a `Default` the tuple itself has not.
type Wide = (u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8);

macro_rules! defaulting {
    ($option:expr) => {
        $option.map_or(bool::default(), |value| value > 1)
    };
}

macro_rules! spare_width {
    () => {
        7
    };
}

/// A fallback this fixture computes, which is the shape the rule is
/// about: a default the reader has to hold on to while reading on.
fn spare() -> u32 {
    7
}

/// The signed counterpart of `spare`, for the defaults written with a
/// leading minus.
fn spare_offset() -> i32 {
    7
}

/// A type worth a struct literal where it has no `Default`, which is
/// the default shape that pushes the mapper furthest to the right.
struct Layout {
    columns: u32,
    wrap: bool,
}

impl Layout {
    fn fitting(columns: u32) -> Self {
        Layout {
            columns,
            wrap: false,
        }
    }
}

/// An enum covering the three ways a variant is written: a unit
/// variant is a path, where a tuple variant is a call to its
/// constructor and a struct variant is a struct literal.
enum Reach {
    Nowhere,
    Deep(u32),
    Named { levels: u32 },
}

// Bad: the default is a call, so the reader carries it as far as the
// mapper that explains why it is there.
fn computed_default(width: Option<u32>) -> u32 {
    width.map_or(spare(), |width| width.saturating_sub(2))
}

// Bad: `String::new()` is what `String`'s `Default` gives, which the
// fix takes.
fn constructed_default(name: Option<&str>) -> String {
    name.map_or(String::new(), |name| name.to_uppercase())
}

// Bad: `Default::default()` says outright what the fix writes.
fn trait_default(width: Option<u32>) -> u32 {
    width.map_or(Default::default(), |width| width + 1)
}

// Bad, and no fix: `Port::new` hands back `Port(8080)` where
// `Port::default` hands back `Port(0)`, so the call keeps its default
// and the help names the open remedies.
fn constructor_that_is_not_the_default(width: Option<u16>) -> Port {
    width.map_or(Port::new(), Port)
}

// Bad, and no fix: every element of the default is that element's
// `Default`, which is not a `Default` for the tuple, so
// `map_or_default` would not compile.
fn default_without_a_default(width: Option<u8>) -> Wide {
    width.map_or((0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0), |width| {
        (width, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
    })
}

// Bad: a struct literal is a complex default, so the reader takes in
// every field of it before meeting the mapper.
fn struct_literal_default(width: Option<u32>) -> Layout {
    width.map_or(
        Layout {
            columns: 80,
            wrap: true,
        },
        Layout::fitting,
    )
}

// Bad: a tuple literal is a complex default, however short.
fn tuple_default(width: Option<u32>) -> (u32, bool) {
    width.map_or((0, false), |width| (width, true))
}

// Bad: a tuple variant of an enum is a call to its constructor.
fn tuple_variant_default(width: Option<u32>) -> Reach {
    width.map_or(Reach::Deep(0), Reach::Deep)
}

// Bad: a struct variant of an enum is a struct literal.
fn struct_variant_default(width: Option<u32>) -> Reach {
    width.map_or(Reach::Named { levels: 0 }, Reach::Deep)
}

// Bad: `format!` builds its value through an expansion, so what the
// reader meets is a macro call.
fn format_default(name: Option<&str>, missing: &str) -> String {
    name.map_or(format!("<{missing}>"), |name| name.to_uppercase())
}

// Bad: `format_args!` is a macro call like any other, whatever it
// expands to.
fn format_args_default(name: Option<&str>) -> String {
    std::fmt::format(name.map_or(format_args!("<none>"), |_| format_args!("<some>")))
}

// Bad: a JSON literal is the everyday default that is a macro call,
// and the reader takes in the whole block of it before the mapper.
fn json_default(name: Option<&str>) -> serde_json::Value {
    name.map_or(serde_json::json!({ "name": "none" }), |name| {
        serde_json::json!({ "name": name })
    })
}

// Bad: a default written as a macro call is a call to the reader,
// whatever it expands to, so the exemption does not reach it.
fn macro_call_default(width: Option<u32>) -> u32 {
    width.map_or(spare_width!(), |width| width + 1)
}

// Bad: the negation is over a call rather than a literal, so the
// exemption does not reach it.
fn negated_call_default(offset: Option<i32>) -> i32 {
    offset.map_or(-spare_offset(), |offset| offset + 1)
}

// Bad: a `&Option<_>` receiver resolves `map_or` through a deref,
// since the payload is `Copy`.
fn through_a_reference(width: &Option<u32>) -> u32 {
    width.map_or(spare(), |width| width + 1)
}

// Bad: a call written as a macro argument carries the author's own
// span, so it is theirs to fix.
fn inside_a_macro_argument(width: Option<u32>) {
    assert!(width.map_or(bool::default(), |width| width > 2));
}

// Not flagged: a literal default is one value, and the reader holding
// it as far as the mapper costs nothing.
fn literal_default(width: Option<u32>) -> u32 {
    width.map_or(80, |width| width.saturating_sub(2))
}

// Not flagged: a negated numeric literal is a literal to the reader,
// and an operator over one only to the parser.
fn negated_literal_default(offset: Option<i32>) -> i32 {
    offset.map_or(-1, |offset| offset + 1)
}

// Not flagged: `None` is a path, and the exemption takes paths.
fn path_default(width: Option<u32>) -> Option<u32> {
    width.map_or(None, |width| width.checked_add(1))
}

// Not flagged: a unit variant of an enum is a path, where the other
// two ways of writing a variant are not.
fn unit_variant_default(width: Option<u32>) -> Reach {
    width.map_or(Reach::Nowhere, Reach::Deep)
}

// Not flagged: a qualified path is a path.
fn qualified_path_default(width: Option<u32>) -> u32 {
    width.map_or(u32::MIN, |width| width + 1)
}

// Not flagged: a binding named on its own is a path.
fn identifier_default(width: Option<u32>, fallback: u32) -> u32 {
    width.map_or(fallback, |width| width + 1)
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

// Good: a `match` names both cases and leaves the default
// unevaluated where there is a value.
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
    slot.map_or(u32::default(), |value| value + 1)
}

// Not flagged: the call belongs to the `defaulting!` definition
// rather than to this line.
fn from_an_expansion(width: Option<u32>) -> bool {
    defaulting!(width)
}

fn main() {}
