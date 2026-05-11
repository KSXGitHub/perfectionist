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

// Good: `pub(crate)` enum is not externally reachable, so default
// `require_for = "pub"` skips it.
pub(crate) enum CrateLocalError {
    Variant,
}

// Good: `pub` enum buried inside a non-`pub` module is not
// externally reachable, so default `require_for = "pub"` skips it.
mod private_module {
    pub enum UnreachableError {
        Variant,
    }
}

// Good: name does not end in `Error` and the type does not implement
// `std::error::Error`.
pub enum SomethingElse {
    Variant,
}

// Bad: name does not end in `Error`, but the type implements
// `std::error::Error` — caught by the trait-impl branch of the
// "looks like an error" predicate.
pub enum BadlyNamed {
    Variant,
}

impl std::fmt::Display for BadlyNamed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("badly named")
    }
}

impl std::fmt::Debug for BadlyNamed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BadlyNamed")
    }
}

impl std::error::Error for BadlyNamed {}

// Good: struct with two fields is not "sum-like", so the struct half
// of the rule does not apply, even though the name ends in `Error`.
pub struct NotSumLikeError(pub ParseKind, pub u32);

// Good: struct whose single field is not an enum is not "sum-like".
pub struct NotEnumWrapperError(pub u32);

// Bad: type-aliased sum-like wrapper. The field's source-level type
// is `KindAlias`, but it resolves through the alias to `ParseKind`
// (an enum), so the SemVer concern still applies.
pub type KindAlias = ParseKind;
pub struct AliasedError(pub KindAlias);

fn main() {}
