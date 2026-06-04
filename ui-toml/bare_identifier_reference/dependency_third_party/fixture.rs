// aux-build:dependency.rs
#![allow(dead_code, reason = "ui fixture")]

extern crate dependency;

use dependency::DepThing;

pub struct LocalThing;

/// Mentions `LocalThing` (first-party) and `DepThing` (a third-party
/// dependency).
pub fn doc(_: DepThing, _: LocalThing) {}

fn main() {}
