//! Walk the re-parsed crate AST for formatting attributes that restate
//! what their derive already does.
//!
//! The walk runs on freshly re-parsed module ASTs (see
//! [`crate::module_reparse`]) rather than on the HIR, because the rule
//! needs the written `#[derive(...)]` list to know which formatting
//! trait is being implemented, and that attribute is consumed during
//! macro expansion. Re-parsing also reaches every separate-file
//! submodule, which a pre-expansion `EarlyLintPass` would not.

use super::formatting_traits::{FormattingTrait, formatting_trait};
use super::forward_template::{
    FieldReference, Fix, Forward, lone_forward, parse_call, template_literal,
};
use crate::attr_tokens::{AttributeCall, attribute_calls_of, is_cfg_gated};
use crate::derive_list::derive_names;
use crate::module_reparse::SpanRange;
use rustc_ast::{
    Attribute, Block, Crate, EnumDef, Expr, ExprKind, Item, ItemKind, ModKind, StmtKind,
    VariantData,
};
use rustc_span::{Span, Symbol};
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
    /// the suggestion deletes when there is one.
    pub(super) attribute: Span,
    pub(super) kind: ForwardKind,
    /// Whether the whole attribute may be deleted outright, or only
    /// warned about (a stray positional index that may name a forgotten
    /// argument).
    pub(super) fix: Fix,
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
            ItemKind::Struct(ident, _, data) if !is_cfg_gated(&item.attrs) => {
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
            ItemKind::Enum(ident, _, def) if !is_cfg_gated(&item.attrs) => {
                check_enum(item, ident.span, def, violations);
            }
            // Descend into inline `mod { ... }` bodies, but only those
            // live in the compiled crate. A re-parse keeps cfg-disabled
            // inline modules, which have no HIR node and so could not be
            // silenced by a local `#[allow]`.
            ItemKind::Mod(_, _, ModKind::Loaded(items, _, spans))
                if live_module_spans.contains(&(spans.inner_span.lo(), spans.inner_span.hi())) =>
            {
                walk_items(items, live_module_spans, violations);
            }
            // A container can also be declared inside a function body
            // or a `const _: () = { ... }` block. Neither is a module,
            // so neither needs the `live_module_spans` guard — an
            // unbuilt one has no HIR node and is dropped at emit time.
            ItemKind::Fn(func) => {
                if let Some(body) = &func.body {
                    walk_block(body, live_module_spans, violations);
                }
            }
            ItemKind::Const(item) => {
                walk_initialiser(item.rhs_kind.expr(), live_module_spans, violations);
            }
            ItemKind::Static(item) => {
                walk_initialiser(item.expr.as_deref(), live_module_spans, violations);
            }
            _ => {}
        }
    }
}

fn walk_block(
    block: &Block,
    live_module_spans: &HashSet<SpanRange>,
    violations: &mut Vec<Violation>,
) {
    for stmt in &block.stmts {
        match &stmt.kind {
            StmtKind::Item(item) => {
                walk_items(core::slice::from_ref(item), live_module_spans, violations);
            }
            StmtKind::Expr(expr) | StmtKind::Semi(expr) => {
                walk_initialiser(Some(expr), live_module_spans, violations);
            }
            _ => {}
        }
    }
}

fn walk_initialiser(
    expr: Option<&Expr>,
    live_module_spans: &HashSet<SpanRange>,
    violations: &mut Vec<Violation>,
) {
    if let Some(Expr {
        kind: ExprKind::Block(block, _),
        ..
    }) = expr
    {
        walk_block(block, live_module_spans, violations);
    }
}

fn check_enum(item: &Item, anchor: Span, def: &EnumDef, violations: &mut Vec<Violation>) {
    let derives = derive_names(&item.attrs);
    if derives.is_empty() {
        return;
    }
    // Any enum-level template shadows its variants' own attributes.
    //
    // A template that does not mention `{_variant}` is simply what a
    // variant falls back to once its own attribute is gone. One that
    // *does* mention it looks safe — the variant is wrapped either way
    // — but only for some traits: under a wrapping template
    // `derive_more` abandons the transparent path, and for `Pointer` it
    // then dereferences the field, so the wrapped form prints the
    // pointee's address and the deleted form prints the binding's.
    // Aliasing the placeholder (`#[display("{_variant}", _variant = 1)]`)
    // makes the template replacing again. Both are narrow, and reasoning
    // about which combinations survive has been wrong more than once, so
    // the enum-level template is treated as shadowing regardless of what
    // it says. Gated templates count too: one that applies only under
    // some `cfg` still shadows its variants there.
    //
    // The cost is a missed diagnostic on a variant under a wrapping
    // template; the enum-level attribute is still flagged on its own
    // terms, and deleting it re-exposes the variant.
    let mut shadowed: HashSet<&'static str> = HashSet::new();
    let calls = attribute_calls_of(&item.attrs);
    let configured = non_template_helpers(&calls);
    for call in &calls {
        let Some(formatting) = derived_formatting_trait(call, &derives) else {
            continue;
        };
        // Keyed on the template literal alone rather than on a full
        // parse: an enum-level template whose arguments this rule
        // cannot read is still a template, and still governs its
        // variants' formatting.
        if template_literal(call.tokens).is_none() {
            continue;
        }
        shadowed.insert(formatting.attribute);
        // The enum-level counterpart of the single-field trigger: a
        // template that is nothing *but* `{_variant}` is exactly what
        // `derive_more` does with no enum-level template at all.
        if call.gated || configured.contains(&call.name) {
            continue;
        }
        if parse_call(call.tokens)
            .and_then(|parsed| lone_forward(&parsed, formatting))
            .is_some_and(|forward| forward.target == Forward::Variant)
        {
            // `{_variant}` is a named placeholder, never a stray index,
            // so the enum-level forward is always a clean deletion.
            violations.push(violation(
                anchor,
                call,
                formatting,
                ForwardKind::Variant,
                Fix::Delete,
            ));
        }
    }
    for variant in &def.variants {
        if is_cfg_gated(&variant.attrs) {
            continue;
        }
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
    let calls = attribute_calls_of(attrs);
    let configured = non_template_helpers(&calls);
    for call in &calls {
        // A gated attribute is read nowhere but the enum-level shadow
        // scan: the field count it would be checked against need not
        // hold under every configuration.
        if call.gated || configured.contains(&call.name) {
            continue;
        }
        let Some(formatting) = derived_formatting_trait(call, derives) else {
            continue;
        };
        if shadowed.contains(formatting.attribute) {
            continue;
        }
        let Some(parsed) = parse_call(call.tokens) else {
            continue;
        };
        if let Some(forward) = lone_forward(&parsed, formatting)
            && forward.target == Forward::Field(sole_field)
        {
            violations.push(violation(
                anchor,
                call,
                formatting,
                ForwardKind::SingleField,
                forward.fix,
            ));
        }
    }
}

/// The helper attributes the node configures through one of
/// `derive_more`'s non-template shapes — `#[display(bound(...))]` and
/// its `bounds(...)` / `where(...)` spellings, `rename_all = "..."`.
///
/// A helper named here is never flagged on this node. `derive_more`
/// folds a `bound(...)` predicate into the impl only on the branch
/// where a format attribute is present, so deleting the template would
/// silently drop the predicates the user wrote beside it — a change to
/// the generated impl, not the no-op the fix promises.
fn non_template_helpers(calls: &[AttributeCall<'_>]) -> HashSet<Symbol> {
    calls
        .iter()
        .filter(|call| template_literal(call.tokens).is_none())
        .map(|call| call.name)
        .collect()
}

fn violation(
    anchor: Span,
    call: &AttributeCall<'_>,
    formatting: &'static FormattingTrait,
    kind: ForwardKind,
    fix: Fix,
) -> Violation {
    Violation {
        anchor,
        attribute: call.span,
        kind,
        fix,
        attribute_name: formatting.attribute,
        derive_name: formatting.derive,
    }
}

/// The formatting trait `call` configures. `None` unless the attribute
/// names one of the formatting helpers *and* the container derives the
/// matching trait — a helper attribute means nothing without the derive
/// that declares it.
fn derived_formatting_trait(
    call: &AttributeCall<'_>,
    derives: &HashSet<Symbol>,
) -> Option<&'static FormattingTrait> {
    let formatting = formatting_trait(call.name)?;
    derives
        .contains(&Symbol::intern(formatting.derive))
        .then_some(formatting)
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
