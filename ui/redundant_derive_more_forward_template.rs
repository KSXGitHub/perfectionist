// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
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

// Good: `derive_more` resolves a bare field name to the field, but wraps
// any other expression as `&(...)` and infers no bound from it, so
// deleting these would rewrite the generated body — and, on a generic
// container, add a bound the expression form never contributed.
#[derive(Display)]
#[display("{}", self.0)]
struct SelfIndexed(String);

// Good: the same, spelled with a named field.
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

// Bad: the enum-level `{_variant}` restates what the derive does when
// an enum carries no shared template at all. Only that attribute is
// reported; a variant sitting under an enum-level template is never
// flagged.
#[derive(Display)]
#[display("{_variant}")]
enum Transparent {
    #[display("{_0}")]
    Text(String),
}

// Good: a variant under an enum-level template is never flagged. Under
// a wrapping template `derive_more` leaves the transparent path, and
// whether the deletion is still a no-op then depends on which trait is
// derived, so the rule declines the shape rather than splitting by
// trait.
#[derive(Display)]
#[display("wrapped: {_variant}")]
enum Wrapped {
    #[display("{_0}")]
    Inner(String),
}

// Good: with `Pointer` the wrapping template makes `derive_more`
// dereference the field, so deleting the variant attribute changes the
// printed address from the pointee's to the binding's.
#[derive(derive_more::Pointer)]
#[pointer("p: {_variant}")]
enum WrappedPointer {
    #[pointer("{_0:p}")]
    Inner(Box<u32>),
}

// Good: aliasing the placeholder makes a `{_variant}` template
// replacing, so the variant would fall back to it.
#[derive(Display)]
#[display("{_variant}", _variant = 1)]
enum AliasedVariant {
    #[display("{_0}")]
    Inner(String),
}

// Bad: a type parameter is no reason to bail, so both variants are
// reported. Either spelling emits the same `Number: Display` predicate.
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

// Bad: only the second variant is reported. The expectation on the
// first resolves to that variant alone, which is what a per-variant
// suppression has to do.
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

// Bad: a lone unadorned placeholder takes derive_more's transparent
// path, which is what the attribute-less derive emits too.
#[derive(derive_more::Pointer)]
#[pointer("{_0:p}")]
struct Address(&'static u32);

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

// Bad: `{}` and `{0}` are the implicit and explicit spellings of the
// same first argument.
#[derive(Display)]
#[display("{0}", _0)]
struct ExplicitPositional(String);

// Bad: `{1}` forwards to the sole argument all the same — derive_more's
// transparent path throws the index away — so this is redundant and
// compiles identically to the bare derive. But the index names an
// argument that was never supplied, so the rule warns without an autofix:
// the fix could be to delete the attribute or to supply the argument.
#[derive(Display)]
#[display("{1}", _0)]
struct MismatchedIndex(String);

// Good: deleting the variant attribute would fall back to `"unknown"`.
#[derive(Display)]
#[display("unknown")]
enum Opaque {
    #[display("{_0}")]
    Known(String),
}

// Good: the enum-level template takes arguments this rule does not
// read, but it is still a template, and still replaces the variant's
// formatting.
#[derive(Display)]
#[display("{a}-{b}", a = 1, b = 2)]
enum UnreadableShared {
    #[display("{_0}")]
    Known(String),
}

// Good: a unit variant has nothing to forward to.
#[derive(Display)]
enum UnitVariant {
    #[display("idle")]
    Idle,
}

// Good: the container may not be in the compiled crate at all, and one
// that is not has no HIR node to anchor a finding at — an `#[allow]` on
// the item could not silence one.
#[cfg(any())]
#[derive(Display)]
#[display("{_0}")]
struct NotBuilt(String);

mod inline_module {
    // Good: the enclosing module is live, so a finding here would
    // anchor at it rather than fall back to the crate root.
    #[cfg(any())]
    #[derive(super::Display)]
    #[display("{_0}")]
    struct NotBuiltNested(String);
}

// Good: the enum itself is built and holds nothing redundant; the
// declined subject is the disabled variant inside it.
#[derive(Display)]
enum DisabledVariant {
    // Good: a gated variant is declined for the same reason, and the
    // enclosing enum is live, so a finding would anchor at it.
    #[cfg(any())]
    #[display("{_0}")]
    Gone(String),
    Kept,
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

// Good: derive_more folds `bound(...)` into the impl only while a
// template is present, so deleting the template here would silently
// drop the predicate. The `where(...)` spelling is the same attribute.
#[derive(Display)]
#[display("{_0}")]
#[display(bound(Inner: Display))]
struct BoundBesideTemplate<Inner>(Inner);

// Good: `where(...)` is the same attribute under another spelling.
#[derive(Display)]
#[display("{_0}")]
#[display(where(Inner: Display))]
struct WhereBesideTemplate<Inner>(Inner);

// Good: a variant carrying its own `bound(...)` beside the template.
#[derive(Display)]
enum VariantBound<Inner> {
    #[display("{_0}")]
    #[display(bound(Inner: Display))]
    Only(Inner),
}

// Good: `derive_more` 0.99's `fmt = "..."` shape has no leading string
// literal, so it never reaches the trigger.
#[derive(Display)]
#[display(fmt = "{}", _0)]
struct LegacyShape(String);

// Bad: a container declared inside a function body is reached too.
fn local_container() {
    #[derive(Display)]
    #[display("{_0}")]
    struct Local(String);
}

// Bad: and one inside a `const _: () = { ... }` block.
const _: () = {
    #[derive(Display)]
    #[display("{_0}")]
    struct InConstBlock(String);
};

// Not a subject: `main` is what makes this fixture a runnable crate.
fn main() {}
