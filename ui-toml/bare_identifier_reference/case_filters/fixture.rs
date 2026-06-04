#![allow(dead_code, non_camel_case_types, reason = "ui fixture")]

pub struct PascalThing;

pub const UPPER_THING: u8 = 0;

pub fn snake_thing() {}

pub struct Mixed_Thing;

/// Mentions `PascalThing`, `UPPER_THING`, and `snake_thing` (all
/// suppressed by the three case knobs), plus the non-conformist
/// `Mixed_Thing`, which is flagged regardless.
pub fn doc() {}

fn main() {}
