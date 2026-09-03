//! Parsing a formatting attribute's tokens into a template plus at most
//! one argument, and deciding whether that template is nothing but the
//! forward the derive already performs.

use super::formatting_traits::FormattingTrait;
use crate::attr_tokens::{ident_name, split_top_level_commas, str_literal};
use crate::format_template::{Placeholder, Segment, parse_template};
use rustc_ast::token::TokenKind;
use rustc_ast::tokenstream::{TokenStream, TokenTree};
use rustc_span::Symbol;

/// A formatting attribute's contents: the template literal, and the one
/// argument that may follow it.
pub(super) struct ParsedCall {
    template: String,
    argument: Option<TemplateArgument>,
}

/// An argument written after the template, in either the positional
/// (`#[display("{}", _0)]`) or the named (`#[display("{x}", x = _0)]`)
/// form.
struct TemplateArgument {
    /// The `x` of `x = _0`; `None` for a positional argument.
    name: Option<Symbol>,
    value: FieldReference,
}

/// How a template names a field of the container it sits on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FieldReference {
    /// A tuple field by index, written `_0`.
    Index(usize),
    /// A named field, unraw-ed — `{type}` names `r#type`.
    Name(Symbol),
}

/// The template literal a formatting attribute opens with, whatever
/// follows it. `None` when the attribute is not a template at all:
/// `#[display(bound(...))]` (also spelled `bounds(...)` / `where(...)`)
/// and `#[display(rename_all = "...")]` are the shapes `derive_more`
/// parses as alternatives to a template, and `derive_more` 0.99's
/// `#[display(fmt = "...")]` is a third. None of them opens with a lone
/// string literal, which is what rules them out here.
///
/// Separate from [`parse_call`] because the two questions differ. The
/// enum-level shadow scan needs to know that an attribute *is* a
/// template even when its arguments are in a shape this module cannot
/// read: such a template still replaces its variants' formatting, so
/// treating it as absent would flag a variant whose attribute is not
/// removable.
pub(super) fn template_literal(tokens: &TokenStream) -> Option<String> {
    let groups = split_top_level_commas(tokens);
    match groups.first()?.as_slice() {
        [tree] => str_literal(tree),
        _ => None,
    }
}

/// Parse a formatting attribute's tokens into its template and
/// argument. `None` when it is not a template (see
/// [`template_literal`]), when it carries more than one argument, or
/// when its argument is in a shape this module does not read.
pub(super) fn parse_call(tokens: &TokenStream) -> Option<ParsedCall> {
    let template = template_literal(tokens)?;
    let mut groups = split_top_level_commas(tokens).into_iter().skip(1);
    // A trailing comma leaves an empty final group; anything beyond one
    // argument cannot be consumed by a lone placeholder, so bail.
    let argument = match groups.next() {
        None => None,
        Some(group) if group.is_empty() => None,
        Some(group) => Some(take_argument(&group)?),
    };
    if groups.any(|group| !group.is_empty()) {
        return None;
    }
    Some(ParsedCall { template, argument })
}

/// What a template forwards to, when it is nothing but one unadorned
/// placeholder selecting `formatting`'s trait. `None` for every other
/// template — one with literal text, a second placeholder, an escaped
/// brace, a fill / width / precision, or a placeholder selecting a
/// different trait than the derive implements.
pub(super) fn lone_forward(call: &ParsedCall, formatting: &FormattingTrait) -> Option<LoneForward> {
    let segments = parse_template(&call.template)?;
    let [Segment::Placeholder(placeholder)] = segments.as_slice() else {
        return None;
    };
    if placeholder.spec() != formatting.spec_type {
        return None;
    }
    resolve(placeholder, call.argument.as_ref())
}

/// A lone forwarding template: what it forwards to, and whether the
/// whole attribute can be deleted outright or only warned about.
pub(super) struct LoneForward {
    pub(super) target: Forward,
    pub(super) fix: Fix,
}

/// The value a lone placeholder interpolates.
#[derive(PartialEq, Eq)]
pub(super) enum Forward {
    /// A field of the container the attribute sits on.
    Field(FieldReference),
    /// `{_variant}` — the enum-level stand-in for whatever each variant
    /// is formatted with.
    Variant,
}

/// Whether a recognised forward is safe to rewrite away, or only to
/// warn about.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Fix {
    /// The template names the value unambiguously — `{}`, `{0}`, `{_0}`,
    /// `{field}`, a named argument, or `{_variant}`. Deleting the whole
    /// attribute is a `MachineApplicable` no-op.
    Delete,
    /// The template forwards to the sole field via an explicit positional
    /// index `>= 1`. `derive_more`'s transparent path throws the index
    /// away, so the output is identical to the derive's own forward and
    /// the code even compiles — but the index names an argument that was
    /// never supplied, which reads as a forgotten one. Redundant as
    /// written, yet the intended fix might be to *supply* the argument
    /// rather than delete it, so warn without offering an autofix.
    WarnOnly,
}

/// Resolve a placeholder's argument against the attribute's own
/// argument list. With no argument supplied the placeholder names the
/// value directly (`{_0}`); with one supplied the placeholder names the
/// argument, which in turn names the value (`{}`, `_0`).
fn resolve(
    placeholder: &Placeholder<'_>,
    supplied: Option<&TemplateArgument>,
) -> Option<LoneForward> {
    let Some(supplied) = supplied else {
        let target = match placeholder.argument {
            // `{}` with nothing to interpolate is not a forward — it is
            // not even a template `derive_more` accepts.
            "" => return None,
            VARIANT_PLACEHOLDER => Forward::Variant,
            name => Forward::Field(field_reference(name)?),
        };
        return Some(LoneForward {
            target,
            fix: Fix::Delete,
        });
    };
    let fix = match supplied.name {
        // A named argument (`#[display("{x}", x = _0)]`) is a clean
        // forward when the placeholder names it.
        Some(name) => (placeholder.argument == name.as_str()).then_some(Fix::Delete)?,
        None => match positional_index(placeholder.argument)? {
            // `{}` and `{0}` are the implicit and explicit spellings of
            // "the first (and only) argument".
            0 => Fix::Delete,
            // `{1}`, `{2}`, ... — see [`Fix::WarnOnly`].
            _ => Fix::WarnOnly,
        },
    };
    Some(LoneForward {
        target: Forward::Field(supplied.value),
        fix,
    })
}

/// The positional index a placeholder argument denotes: `{}` is the
/// implicit first (index 0), `{0}` / `{1}` / ... are explicit. `None`
/// for a named placeholder, which a positional argument cannot satisfy.
fn positional_index(argument: &str) -> Option<usize> {
    if argument.is_empty() {
        Some(0)
    } else {
        argument.parse().ok()
    }
}

/// `derive_more`'s enum-level stand-in for a variant's own formatting.
const VARIANT_PLACEHOLDER: &str = "_variant";

/// Read a field reference out of a placeholder argument or a bare
/// identifier argument. `None` for an explicit positional index
/// (`{0}`), which names an argument rather than a field.
fn field_reference(text: &str) -> Option<FieldReference> {
    if let Some(index) = text.strip_prefix('_').and_then(|rest| rest.parse().ok()) {
        return Some(FieldReference::Index(index));
    }
    if text.starts_with(|first: char| first.is_ascii_digit()) {
        return None;
    }
    Some(FieldReference::Name(Symbol::intern(text)))
}

/// Parse one comma-separated argument group as `[<name> =] <value>`.
fn take_argument(group: &[&TokenTree]) -> Option<TemplateArgument> {
    if let [name, equals, value @ ..] = group
        && let Some(name) = ident_name(name)
        && matches!(equals, TokenTree::Token(token, _) if token.kind == TokenKind::Eq)
    {
        return Some(TemplateArgument {
            name: Some(name),
            value: take_value(value)?,
        });
    }
    Some(TemplateArgument {
        name: None,
        value: take_value(group)?,
    })
}

/// Parse an argument's value. Only a bare identifier counts — `_0`,
/// `message` — because that is the only spelling `derive_more`
/// resolves to the field itself.
///
/// Every other expression, `self.0` and `self.message` included, it
/// wraps as `&(<expr>)` and infers no formatting bound from. Deleting
/// such an attribute therefore rewrites the generated body and, on a
/// generic container, adds the bound the expression form never
/// contributed — so the spelling is left unflagged rather than fixed
/// into a different impl.
fn take_value(tokens: &[&TokenTree]) -> Option<FieldReference> {
    match tokens {
        [only] => field_reference(ident_name(only)?.as_str()),
        _ => None,
    }
}
