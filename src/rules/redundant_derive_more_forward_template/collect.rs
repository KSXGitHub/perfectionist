//! Walk the re-parsed crate AST for formatting attributes that restate
//! what their derive already does.
//!
//! The walk runs on freshly re-parsed module ASTs (see
//! [`crate::module_reparse`]) rather than on the HIR, because the rule
//! needs the written `#[derive(...)]` list to know which formatting
//! trait is being implemented, and that attribute is consumed during
//! macro expansion. Re-parsing also reaches every separate-file
//! submodule, which a pre-expansion `EarlyLintPass` would not.

use super::attrs::{
    AttributeCall, FieldReference, FormattingTrait, Forward, ParsedCall, attribute_calls,
    formatting_trait, lone_forward, parse_call, wraps_variants,
};
use crate::module_reparse::SpanRange;
use rustc_ast::tokenstream::TokenStream;
use rustc_ast::{
    Attribute, Crate, EnumDef, Item, ItemKind, MetaItemInner, MetaItemKind, ModKind, VariantData,
};
use rustc_span::{Span, Symbol, sym};
use std::collections::HashSet;

/// One removable formatting attribute.
pub(super) struct Violation {
    /// Span of the identifier of the node the attribute sits on — the
    /// struct, the enum, or the variant. The diagnostic anchors here so
    /// a per-variant `#[allow]` silences just that variant while one on
    /// the enum still covers them all. The identifier is used rather
    /// than the node's own span because it is inside both the re-parsed
    /// node and its HIR counterpart, whatever either does with the
    /// node's leading attributes.
    pub(super) anchor: Span,
    /// The whole attribute — the diagnostic's primary span, and what
    /// the suggestion deletes.
    pub(super) attribute: Span,
    pub(super) kind: ForwardKind,
    /// The helper attribute's name, for the diagnostic.
    pub(super) attribute_name: &'static str,
    /// The derive whose default the template restates.
    pub(super) derive_name: &'static str,
}

/// Which of the two shapes fired, for the diagnostic's wording.
pub(super) enum ForwardKind {
    /// A single-field struct or variant whose template interpolates
    /// that field and nothing else.
    SingleField,
    /// An enum-level template that is exactly `{_variant}`.
    Variant,
}

/// Collect every removable formatting attribute in `crates`.
pub(super) fn collect_violations(
    crates: &[Crate],
    live_module_spans: &HashSet<SpanRange>,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    for krate in crates {
        walk_items(&krate.items, live_module_spans, &mut violations);
    }
    violations
}

fn walk_items(
    items: &[Box<Item>],
    live_module_spans: &HashSet<SpanRange>,
    violations: &mut Vec<Violation>,
) {
    for item in items {
        match &item.kind {
            ItemKind::Struct(ident, _, data) => {
                let derives = derive_names(&item.attrs);
                check_container(
                    &item.attrs,
                    ident.span,
                    data,
                    &derives,
                    &HashSet::new(),
                    violations,
                );
            }
            ItemKind::Enum(ident, _, def) => check_enum(item, ident.span, def, violations),
            // Descend into inline `mod { ... }` bodies, but only those
            // live in the compiled crate. A re-parse keeps cfg-disabled
            // inline modules, which have no HIR node and so could not be
            // silenced by a local `#[allow]`.
            ItemKind::Mod(_, _, ModKind::Loaded(items, _, spans))
                if live_module_spans.contains(&(spans.inner_span.lo(), spans.inner_span.hi())) =>
            {
                walk_items(items, live_module_spans, violations);
            }
            _ => {}
        }
    }
}

fn check_enum(item: &Item, anchor: Span, def: &EnumDef, violations: &mut Vec<Violation>) {
    let derives = derive_names(&item.attrs);
    if derives.is_empty() {
        return;
    }
    // An enum-level template that does *not* mention `{_variant}` is
    // what a variant falls back to once its own attribute is gone, so a
    // variant attribute under one is not removable. Gated enum-level
    // templates count too: one that applies only under some `cfg` still
    // shadows its variants there.
    let mut shadowed: HashSet<&'static str> = HashSet::new();
    for call in attribute_calls_of(&item.attrs) {
        let Some((formatting, parsed)) = formatting_call(&call, &derives) else {
            continue;
        };
        if !wraps_variants(&parsed) {
            shadowed.insert(formatting.attribute);
            continue;
        }
        // The enum-level counterpart of the single-field trigger: a
        // template that is nothing *but* `{_variant}` is exactly what
        // `derive_more` does with no enum-level template at all.
        if !call.gated && lone_forward(&parsed, formatting) == Some(Forward::Variant) {
            violations.push(violation(anchor, &call, formatting, ForwardKind::Variant));
        }
    }
    for variant in &def.variants {
        check_container(
            &variant.attrs,
            variant.ident.span,
            &variant.data,
            &derives,
            &shadowed,
            violations,
        );
    }
}

/// Check one single-field container — a struct, or an enum variant —
/// for a formatting attribute that only restates the forward its derive
/// already performs.
///
/// `derives` is the derive list that governs the container: its own for
/// a struct, the enclosing enum's for a variant. `shadowed` names the
/// helper attributes whose enum-level template would take over if the
/// variant's own were deleted, and is empty for a struct.
fn check_container(
    attrs: &[Attribute],
    anchor: Span,
    data: &VariantData,
    derives: &HashSet<Symbol>,
    shadowed: &HashSet<&'static str>,
    violations: &mut Vec<Violation>,
) {
    if derives.is_empty() {
        return;
    }
    // A `#[cfg(...)]`-gated field makes the field count differ between
    // configurations, so the single-field premise cannot be checked.
    if data.fields().iter().any(|field| is_cfg_gated(&field.attrs)) {
        return;
    }
    let Some(sole_field) = sole_field_reference(data) else {
        return;
    };
    for call in attribute_calls_of(attrs) {
        // A gated attribute is read nowhere but the enum-level shadow
        // scan: the field count it would be checked against need not
        // hold under every configuration.
        if call.gated {
            continue;
        }
        let Some((formatting, parsed)) = formatting_call(&call, derives) else {
            continue;
        };
        if shadowed.contains(formatting.attribute) {
            continue;
        }
        if lone_forward(&parsed, formatting) == Some(Forward::Field(sole_field)) {
            violations.push(violation(
                anchor,
                &call,
                formatting,
                ForwardKind::SingleField,
            ));
        }
    }
}

fn violation(
    anchor: Span,
    call: &AttributeCall<'_>,
    formatting: &'static FormattingTrait,
    kind: ForwardKind,
) -> Violation {
    Violation {
        anchor,
        attribute: call.span,
        kind,
        attribute_name: formatting.attribute,
        derive_name: formatting.derive,
    }
}

/// The formatting trait `call` configures, and its parsed contents.
/// `None` unless the attribute names one of the formatting helpers, the
/// container derives the matching trait, and the attribute holds a
/// template rather than one of `derive_more`'s other argument shapes.
fn formatting_call(
    call: &AttributeCall<'_>,
    derives: &HashSet<Symbol>,
) -> Option<(&'static FormattingTrait, ParsedCall)> {
    let formatting = formatting_trait(call.name)?;
    if !derives.contains(&Symbol::intern(formatting.derive)) {
        return None;
    }
    Some((formatting, parse_call(call.tokens)?))
}

/// The field a single-field container's template can name: the index
/// for a tuple field, the (unraw-ed) name for a named one. `None` for a
/// container with zero or more than one field — with more than one the
/// template is mandatory, and with none there is nothing to forward to.
fn sole_field_reference(data: &VariantData) -> Option<FieldReference> {
    let [field] = data.fields() else {
        return None;
    };
    Some(match field.ident {
        Some(ident) => FieldReference::Name(ident.name),
        None => FieldReference::Index(0),
    })
}

fn attribute_calls_of(attrs: &[Attribute]) -> Vec<AttributeCall<'_>> {
    attrs.iter().flat_map(attribute_calls).collect()
}

/// Whether a `#[cfg(...)]` gates the node, including one applied
/// through a `#[cfg_attr(...)]`.
fn is_cfg_gated(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.has_name(sym::cfg)
            || attribute_calls(attr)
                .iter()
                .any(|call| call.name == sym::cfg)
    })
}

/// Final path segment of every derive on the node, including
/// `#[cfg_attr(<cfg>, derive(...))]`-gated ones — a gated derive still
/// governs what its helper attribute means wherever it applies.
///
/// Matching by final segment catches `derive_more::Display`, a plain
/// `Display` imported from `derive_more`, and a same-name re-export; a
/// derive renamed through `use derive_more::Display as D;` is not
/// caught, the same accepted limitation the sibling
/// `perfectionist::unordered_derives` and
/// `perfectionist::clap_help_markdown` already carry.
fn derive_names(attrs: &[Attribute]) -> HashSet<Symbol> {
    let mut names = HashSet::new();
    for call in attribute_calls_of(attrs) {
        if call.name == sym::derive {
            names.extend(derive_entries(call.tokens));
        }
    }
    names
}

/// Final path segment of each entry in a `derive(...)` list.
fn derive_entries(tokens: &TokenStream) -> Vec<Symbol> {
    let Some(entries) = MetaItemKind::list_from_tokens(tokens.clone()) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(MetaItemInner::meta_item)
        .filter_map(|meta| meta.path.segments.last())
        .map(|segment| segment.ident.name)
        .collect()
}
