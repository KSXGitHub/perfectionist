//! Reading a `derive_more` formatting attribute off the re-parsed AST.
//!
//! Three things live here, in the order the rule needs them: the table
//! that ties each formatting derive to its helper attribute and to the
//! format-spec type that selects the same trait ([`FORMATTING_TRAITS`]);
//! the `#[name(...)]` unwrapping that looks through `#[cfg_attr(...)]`
//! ([`attribute_calls`]); and the parse of an attribute's own tokens
//! into a template plus at most one argument ([`parse_call`]).
//!
//! Everything is read from tokens rather than from `meta_item_list`,
//! because the un-inlined argument forms the rule has to recognise are
//! not meta items: `#[display("{}", self.0)]` has an expression where a
//! meta item would have to be.

use crate::format_template::{Placeholder, Segment, parse_template};
use rustc_ast::token::TokenKind;
use rustc_ast::tokenstream::{TokenStream, TokenTree};
use rustc_ast::{AttrArgs, AttrKind, Attribute, LitKind};
use rustc_span::{Span, Symbol, kw, sym};

/// One `derive_more` formatting trait, in the three spellings the rule
/// has to line up: the derive that implements it, the helper attribute
/// that configures it, and the format-spec type that selects it in a
/// placeholder.
pub(super) struct FormattingTrait {
    /// Final path segment of the derive, as written in `#[derive(...)]`.
    pub(super) derive: &'static str,
    /// The derive's helper attribute, always written unqualified.
    pub(super) attribute: &'static str,
    /// The placeholder type that selects this trait — `""` for
    /// `Display`, whose placeholder carries no type at all.
    spec_type: &'static str,
}

/// Every `derive_more` derive whose no-attribute default is a forward
/// to the container's single field.
///
/// `Debug` is deliberately absent: its default is the struct-shaped
/// `Wrapper("inner")` builder output rather than a forward, so a
/// `#[debug("{_0:?}")]` genuinely changes the rendering.
const FORMATTING_TRAITS: &[FormattingTrait] = &[
    FormattingTrait {
        derive: "Binary",
        attribute: "binary",
        spec_type: "b",
    },
    FormattingTrait {
        derive: "Display",
        attribute: "display",
        spec_type: "",
    },
    FormattingTrait {
        derive: "LowerExp",
        attribute: "lower_exp",
        spec_type: "e",
    },
    FormattingTrait {
        derive: "LowerHex",
        attribute: "lower_hex",
        spec_type: "x",
    },
    FormattingTrait {
        derive: "Octal",
        attribute: "octal",
        spec_type: "o",
    },
    FormattingTrait {
        derive: "Pointer",
        attribute: "pointer",
        spec_type: "p",
    },
    FormattingTrait {
        derive: "UpperExp",
        attribute: "upper_exp",
        spec_type: "E",
    },
    FormattingTrait {
        derive: "UpperHex",
        attribute: "upper_hex",
        spec_type: "X",
    },
];

/// The formatting trait whose helper attribute is named `name`.
pub(super) fn formatting_trait(name: Symbol) -> Option<&'static FormattingTrait> {
    FORMATTING_TRAITS
        .iter()
        .find(|entry| name == Symbol::intern(entry.attribute))
}

/// One `#[name(...)]` attribute as written on a node, with any
/// `#[cfg_attr(<cfg>, ...)]` wrapper peeled off.
pub(super) struct AttributeCall<'a> {
    /// Final segment of the attribute's path. A formatting attribute is
    /// a derive helper, so it is only ever written unqualified — but a
    /// non-formatting attribute may not be, and taking the last segment
    /// keeps the comparison honest either way.
    pub(super) name: Symbol,
    /// The attribute's argument tokens, without the delimiters.
    pub(super) tokens: &'a TokenStream,
    /// Whether a `#[cfg_attr(...)]` gates the attribute. A gated
    /// attribute is read but never flagged: the field count it would be
    /// checked against need not hold under every configuration.
    pub(super) gated: bool,
    /// The whole attribute, for the diagnostic and its deletion. For a
    /// gated call this is the enclosing `#[cfg_attr(...)]`, which is
    /// never flagged, so it is never used as a deletion target.
    pub(super) span: Span,
}

/// Every `#[name(...)]` attribute `attr` carries, looking through a
/// `#[cfg_attr(<cfg>, ...)]` gate — and through a nested one, since
/// `cfg_attr(a, cfg_attr(b, ...))` is how `cfg_attr(all(a, b), ...)` is
/// sometimes spelled. An attribute in any other shape (`#[foo]`,
/// `#[foo = "bar"]`, a doc comment) yields nothing.
pub(super) fn attribute_calls(attr: &Attribute) -> Vec<AttributeCall<'_>> {
    let AttrKind::Normal(_) = &attr.kind else {
        return Vec::new();
    };
    let item = attr.get_normal_item();
    let Some(name) = item.path.segments.last().map(|segment| segment.ident.name) else {
        return Vec::new();
    };
    let Some(AttrArgs::Delimited(args)) = item.args.unparsed_ref() else {
        return Vec::new();
    };
    if name == sym::cfg_attr {
        return gated_calls(&args.tokens, attr.span);
    }
    vec![AttributeCall {
        name,
        tokens: &args.tokens,
        gated: false,
        span: attr.span,
    }]
}

/// The attributes a `#[cfg_attr(<cfg>, ...)]` applies, given its
/// argument tokens. The first comma-separated group is the predicate
/// and is skipped; every group after it is an attribute the predicate
/// gates, and is itself unwrapped when it is another `cfg_attr`.
fn gated_calls(tokens: &TokenStream, span: Span) -> Vec<AttributeCall<'_>> {
    let mut calls = Vec::new();
    for group in split_top_level_commas(tokens).into_iter().skip(1) {
        let [name, arguments] = group.as_slice() else {
            continue;
        };
        let (Some(name), TokenTree::Delimited(_, _, _, inner)) = (ident_name(name), arguments)
        else {
            continue;
        };
        if name == sym::cfg_attr {
            calls.extend(gated_calls(inner, span));
        } else {
            calls.push(AttributeCall {
                name,
                tokens: inner,
                gated: true,
                span,
            });
        }
    }
    calls
}

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
    /// A tuple field by index: `_0` as a placeholder argument, `self.0`
    /// as an attribute argument.
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
pub(super) fn lone_forward(call: &ParsedCall, formatting: &FormattingTrait) -> Option<Forward> {
    let segments = parse_template(&call.template)?;
    let [Segment::Placeholder(placeholder)] = segments.as_slice() else {
        return None;
    };
    if placeholder.spec() != formatting.spec_type {
        return None;
    }
    resolve(placeholder, call.argument.as_ref())
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

/// Resolve a placeholder's argument against the attribute's own
/// argument list. With no argument supplied the placeholder names the
/// value directly (`{_0}`); with one supplied the placeholder names the
/// argument, which in turn names the value (`{}`, `_0`).
fn resolve(placeholder: &Placeholder<'_>, supplied: Option<&TemplateArgument>) -> Option<Forward> {
    let Some(supplied) = supplied else {
        return match placeholder.argument {
            // `{}` with nothing to interpolate is not a forward — it is
            // not even a template `derive_more` accepts.
            "" => None,
            VARIANT_PLACEHOLDER => Some(Forward::Variant),
            name => field_reference(name).map(Forward::Field),
        };
    };
    let names_the_argument = match supplied.name {
        Some(name) => placeholder.argument == name.as_str(),
        // `{}` and `{0}` are the implicit and explicit spellings of
        // "the first argument", and `derive_more` compiles both to the
        // same forward. A higher index is deliberately left alone:
        // `derive_more` would still forward to the sole argument, but
        // the bound it infers is then not the one the attribute-less
        // derive infers, so the deletion would not be
        // output-preserving.
        None => matches!(placeholder.argument, "" | "0"),
    };
    names_the_argument.then_some(Forward::Field(supplied.value))
}

/// `derive_more`'s enum-level stand-in for a variant's own formatting.
const VARIANT_PLACEHOLDER: &str = "_variant";

/// Whether a template mentions `{_variant}` anywhere. An enum-level
/// template that does is a wrapper around each variant's own
/// formatting, so a variant's attribute is still removable under it; one
/// that does not replaces the variant's formatting outright, and
/// removing a variant's attribute would change the output to this text.
/// An unparsable template counts as replacing — the conservative answer.
pub(super) fn wraps_variants(template: &str) -> bool {
    parse_template(template).is_some_and(|segments| {
        segments.iter().any(|segment| {
            matches!(segment, Segment::Placeholder(placeholder)
                if placeholder.argument == VARIANT_PLACEHOLDER)
        })
    })
}

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

/// Parse an argument's value. Only the two spellings that name a field
/// are recognised — a bare `_0` / `message`, and `self.0` /
/// `self.message`. The borrowed and dereferenced spellings (`*_0`,
/// `&self.0`) forward identically but are rare, so leaving them out
/// costs only a missed diagnostic.
fn take_value(tokens: &[&TokenTree]) -> Option<FieldReference> {
    match tokens {
        [only] => field_reference(ident_name(only)?.as_str()),
        [receiver, dot, field]
            if ident_name(receiver) == Some(kw::SelfLower)
                && matches!(dot, TokenTree::Token(token, _) if token.kind == TokenKind::Dot) =>
        {
            self_field(field)
        }
        _ => None,
    }
}

/// The field named by the tail of a `self.<field>` argument: an
/// identifier for a named field, an integer literal for a tuple one.
fn self_field(token: &TokenTree) -> Option<FieldReference> {
    if let Some(name) = ident_name(token) {
        return Some(FieldReference::Name(name));
    }
    let index = integer_literal(token)?;
    Some(FieldReference::Index(index))
}

/// Split a token stream into comma-separated groups. A comma inside a
/// delimited group is not top-level, so `cfg_attr`'s `all(a, b)`
/// predicate stays one group.
fn split_top_level_commas(tokens: &TokenStream) -> Vec<Vec<&TokenTree>> {
    let mut groups = vec![Vec::new()];
    for tree in tokens.iter() {
        if matches!(tree, TokenTree::Token(token, _) if token.kind == TokenKind::Comma) {
            groups.push(Vec::new());
        } else {
            groups
                .last_mut()
                .expect("`groups` starts non-empty and only ever grows")
                .push(tree);
        }
    }
    groups
}

fn ident_name(tree: &TokenTree) -> Option<Symbol> {
    let TokenTree::Token(token, _) = tree else {
        return None;
    };
    token.ident().map(|(ident, _raw)| ident.name)
}

/// The cooked value of a string literal token, raw and escaped forms
/// alike — `r"{_0}"` and `"{_0}"` are the same template.
fn str_literal(tree: &TokenTree) -> Option<String> {
    let TokenTree::Token(token, _) = tree else {
        return None;
    };
    let TokenKind::Literal(literal) = token.kind else {
        return None;
    };
    let LitKind::Str(symbol, _style) = LitKind::from_token_lit(literal).ok()? else {
        return None;
    };
    Some(symbol.as_str().to_owned())
}

fn integer_literal(tree: &TokenTree) -> Option<usize> {
    let TokenTree::Token(token, _) = tree else {
        return None;
    };
    let TokenKind::Literal(literal) = token.kind else {
        return None;
    };
    let LitKind::Int(value, _suffix) = LitKind::from_token_lit(literal).ok()? else {
        return None;
    };
    usize::try_from(value.get()).ok()
}
