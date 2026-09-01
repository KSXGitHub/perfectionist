// aux-build:derive_more.rs
//
// UI sweep for `redundant_derive_more_forward_template` under the
// default configuration. Every template that compiles to the forward
// its derive already performs is flagged and offered for deletion;
// every shape where deleting the attribute would change the output is
// left alone.

#![feature(register_tool)]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]
#![allow(dead_code, reason = "ui fixture")]

extern crate derive_more;

use derive_more::{Display, Display as Renamed};

// Bad: a tuple newtype restating the forward.
#[derive(Display)]
#[display("{_0}")]
struct SanitizedHtml(String);

// Bad: the named-field spelling of the same thing.
#[derive(Display)]
#[display("{message}")]
struct Warning {
    message: String,
}

// Bad: the un-inlined positional argument form.
#[derive(Display)]
#[display("{}", _0)]
struct UninlinedPositional(String);

// Bad: the `self.<index>` argument form.
#[derive(Display)]
#[display("{}", self.0)]
struct SelfIndexed(String);

// Bad: the `self.<name>` argument form.
#[derive(Display)]
#[display("{}", self.message)]
struct SelfNamed {
    message: String,
}

// Bad: a named argument bound to the field.
#[derive(Display)]
#[display("{x}", x = _0)]
struct NamedArgument(String);

// Bad: the placeholder argument is unraw-ed, so it names `r#type`.
#[derive(Display)]
#[display("{type}")]
struct RawIdentField {
    r#type: String,
}

// Bad: an empty format spec is still no formatting.
#[derive(Display)]
#[display("{_0:}")]
struct EmptySpec(String);

// Bad: the fully-qualified derive path matches by final segment.
#[derive(derive_more::Display)]
#[display("{_0}")]
struct QualifiedDerive(String);

// Bad: the same shape under a non-`Display` formatting trait.
#[derive(derive_more::LowerHex)]
#[lower_hex("{_0:x}")]
struct Mask(u32);

// Bad: and under another one.
#[derive(derive_more::Binary)]
#[binary("{_0:b}")]
struct Flags(u8);

// Bad: a single-field variant, alongside a variant that does real work.
#[derive(Display)]
enum ParseError {
    #[display("{_0}")]
    Io(String),
    #[display("bad token at offset {_0}")]
    BadToken(usize),
}

// Bad: an enum-level template that is nothing but `{_variant}`.
#[derive(Display)]
#[display("{_variant}")]
enum Status {
    Idle,
    #[display("running for {_0}s")]
    Running(u64),
}

// Bad twice: the enum-level `{_variant}` and the variant it wraps.
#[derive(Display)]
#[display("{_variant}")]
enum Transparent {
    #[display("{_0}")]
    Text(String),
}

// Bad: an enum-level template that mentions `{_variant}` wraps rather
// than replaces, so the variant's own attribute is still removable.
#[derive(Display)]
#[display("wrapped: {_variant}")]
enum Wrapped {
    #[display("{_0}")]
    Inner(String),
}

// Bad twice: a type parameter is no reason to bail. Both spellings emit
// the same `Number: Display` predicate.
#[derive(Display)]
enum StringOrNumber<Number> {
    #[display("{_0}")]
    Text(String),
    #[display("{_0}")]
    Number(Number),
}

// Bad: an attribute sharing its line with the variant loses only
// itself, not the line.
#[derive(Display)]
enum SameLine {
    #[display("{_0}")] Text(String),
}

// Bad once: the expectation on the first variant resolves to that
// variant alone, so the second is still reported.
#[derive(Display)]
enum PerVariantExpect {
    #[cfg_attr(
        dylint_lib = "perfectionist",
        expect(
            perfectionist::redundant_derive_more_forward_template,
            reason = "checks that a per-variant expectation resolves"
        )
    )]
    #[display("{_0}")]
    Silenced(String),
    #[display("{_0}")]
    Reported(String),
}

// Good: `{}` forwards to `Display`, not to `LowerHex`, so the attribute
// changes the rendering.
#[derive(derive_more::LowerHex)]
#[lower_hex("{_0}")]
struct DisplayUnderLowerHex(u32);

// Good: `Debug` does not default to a forward.
#[derive(derive_more::Debug)]
#[debug("{_0:?}")]
struct Payload(Vec<u8>);

// Good: a `Debug` placeholder under a `Display` derive.
#[derive(Display)]
#[display("{_0:?}")]
struct DebugForward(u32);

// Good: the width is applied here rather than passed through.
#[derive(Display)]
#[display("{_0:>8}")]
struct Padded(u32);

// Good: two fields, so the template is mandatory.
#[derive(Display)]
#[display("{_0}")]
struct Pair(u32, u32);

// Good: no field to forward to.
#[derive(Display)]
#[display("unit")]
struct UnitStruct;

// Good: literal text alongside the placeholder.
#[derive(Display)]
#[display("<{_0}>")]
struct Angled(String);

// Good: an escaped brace is literal text too.
#[derive(Display)]
#[display("{{{_0}}}")]
struct Braced(String);

// Good: an explicit positional index names an argument, not the field.
#[derive(Display)]
#[display("{0}", _0)]
struct ExplicitPositional(String);

// Good: deleting the variant attribute would fall back to `"unknown"`.
#[derive(Display)]
#[display("unknown")]
enum Opaque {
    #[display("{_0}")]
    Known(String),
}

// Good: a unit variant has nothing to forward to.
#[derive(Display)]
enum UnitVariant {
    #[display("idle")]
    Idle,
}

// Good: a `cfg`-gated field makes the field count depend on the
// configuration.
#[derive(Display)]
#[display("{_0}")]
struct GatedField(#[cfg(all())] String);

// Good: a template written inside a `cfg_attr` is a bail.
#[derive(Display)]
#[cfg_attr(all(), display("{_0}"))]
struct GatedTemplate(String);

// Good: a derive renamed on import is not matched by final segment.
#[derive(Renamed)]
#[display("{_0}")]
struct RenamedDerive(String);

// Good: `bound(...)` is one of the alternatives to a template, not a
// template.
#[derive(Display)]
#[display(bound(Inner: Display))]
struct Bounded<Inner>(Inner);

// Good: `derive_more` 0.99's `fmt = "..."` shape has no leading string
// literal, so it never reaches the trigger.
#[derive(Display)]
#[display(fmt = "{}", _0)]
struct LegacyShape(String);

fn main() {}
