// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// With `max_fields = 0` every struct with a field is flagged and the
// diagnostic states the count.

// Not flagged: a unit struct has no fields.
struct Unit;

// 1.
struct One {
    field_1: u32,
}

// 3.
struct Three {
    field_1: u32,
    field_2: u32,
    field_3: u32,
}

// 2: a tuple struct's fields count.
struct Pair(u32, u32);

// 1: a generic struct is measured like any other.
struct Wrapper<Value> {
    value: Value,
}

// Not flagged: an enum's struct-like variant is not a struct.
enum Shape {
    Wide { width: u32, height: u32 },
}

// Not flagged: nor is a union.
union Bits {
    signed: i32,
    unsigned: u32,
}

// 1: a struct declared inside a function is measured.
fn holder() {
    struct Local {
        field_1: u32,
    }
}

fn main() {}
