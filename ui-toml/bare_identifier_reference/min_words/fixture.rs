// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
#![allow(dead_code, non_camel_case_types, reason = "ui fixture")]

pub struct Foo;

pub struct FooBarBaz;

pub fn foo_bar() {}

pub fn foo_bar_baz() {}

pub struct Mixed_Thing;

/// Below the threshold and exempt: `Foo` (1 word), `foo_bar` (2).
/// At or above it and checked: `FooBarBaz` (3), `foo_bar_baz` (3).
/// Non-conformist and checked regardless: `Mixed_Thing`.
pub fn doc() {}

fn main() {}
