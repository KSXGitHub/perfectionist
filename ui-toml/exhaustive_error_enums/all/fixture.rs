#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(unknown_lints, reason = "ui fixture")]
#![warn(perfectionist::exhaustive_error_enums)]

// Under `require_for = "all"`, every error-shaped enum is flagged
// regardless of visibility, including module-private ones.

pub enum PublicError {
    Variant,
}

pub(crate) enum CrateLocalError {
    Variant,
}

enum PrivateError {
    Variant,
}

fn main() {}
