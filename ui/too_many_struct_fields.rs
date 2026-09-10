// edition:2024
#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(dead_code, unused, reason = "ui fixture")]

// Bad: eleven fields, one above the default limit.
struct Eleven {
    field_1: u32,
    field_2: u32,
    field_3: u32,
    field_4: u32,
    field_5: u32,
    field_6: u32,
    field_7: u32,
    field_8: u32,
    field_9: u32,
    field_10: u32,
    field_11: u32,
}

// Good: ten is not above the limit.
struct Ten {
    field_1: u32,
    field_2: u32,
    field_3: u32,
    field_4: u32,
    field_5: u32,
    field_6: u32,
    field_7: u32,
    field_8: u32,
    field_9: u32,
    field_10: u32,
}

// Good: the eleven fields split by what they are.
struct Split {
    first: FirstHalf,
    second: SecondHalf,
}

struct FirstHalf {
    field_1: u32,
    field_2: u32,
    field_3: u32,
    field_4: u32,
    field_5: u32,
}

struct SecondHalf {
    field_1: u32,
    field_2: u32,
    field_3: u32,
    field_4: u32,
    field_5: u32,
    field_6: u32,
}

// Bad: a tuple struct is measured too.
struct Tuple(u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32);

// Good: an enum's variants are not measured.
enum Shape {
    Wide {
        field_1: u32,
        field_2: u32,
        field_3: u32,
        field_4: u32,
        field_5: u32,
        field_6: u32,
        field_7: u32,
        field_8: u32,
        field_9: u32,
        field_10: u32,
        field_11: u32,
    },
}

// Good: a struct a macro expands to is not measured.
macro_rules! wide {
    ($name:ident) => {
        struct $name {
            field_1: u32,
            field_2: u32,
            field_3: u32,
            field_4: u32,
            field_5: u32,
            field_6: u32,
            field_7: u32,
            field_8: u32,
            field_9: u32,
            field_10: u32,
            field_11: u32,
        }
    };
}

wide!(FromMacro);

fn main() {}
