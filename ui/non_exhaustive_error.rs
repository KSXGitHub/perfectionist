#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints)]
#![allow(dead_code)]

// Bad: public error enum missing `#[non_exhaustive]`.
pub enum RuntimeError {
    Serialization,
    Io,
}

// Helper enum referenced by the wrapper struct below. It does not end
// in `Error` and is not flagged on its own.
#[non_exhaustive]
pub enum ParseKind {
    Bad,
    Worse,
}

// Bad: public sum-like tuple struct (single field is an enum) whose
// name ends in `Error`.
pub struct ParseError(pub ParseKind);

// Good: already marked `#[non_exhaustive]`.
#[non_exhaustive]
pub enum AlreadyMarkedError {
    Variant,
}

// Good: non-`pub` enum (default `require_for = "pub"` skips this).
enum PrivateError {
    Variant,
}

// Good: name does not end in `Error` and the type does not implement
// `std::error::Error`.
pub enum SomethingElse {
    Variant,
}

// Good: struct with two fields is not "sum-like", so the struct half
// of the rule does not apply, even though the name ends in `Error`.
pub struct NotSumLikeError(pub ParseKind, pub u32);

// Good: struct whose single field is not an enum is not "sum-like".
pub struct NotEnumWrapperError(pub u32);

fn main() {}
