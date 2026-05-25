//! Diagnostic-emission helpers, one per violation shape, plus the
//! `#[error(...)]`-attribute walkers that find them. All carry the
//! help-only `prefer_derive_more_over_thiserror` diagnostic (no
//! `Suggestion`, no `Applicability` — the migration is manual).

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::{Attribute, EnumDef, MetaItemInner, MetaItemKind, VariantData};
use rustc_lint::EarlyContext;
use rustc_span::{Span, Symbol, sym};

use super::PREFER_DERIVE_MORE_OVER_THISERROR;

/// Flag every `#[error(...)]` attribute on a thiserror-derived item,
/// unwrapping `#[cfg_attr(pred, error(...))]` wrappers symmetrically
/// with the derive side.
pub(super) fn flag_error_attrs(cx: &EarlyContext<'_>, attrs: &[Attribute]) {
    // `Symbol::intern("error")` rather than a `kw::` / `sym::`
    // constant because `error` is not a pre-interned compiler
    // symbol. The intern lookup happens once per attribute on
    // thiserror-derived items, which is cheap relative to the
    // surrounding diagnostic emission.
    let error = Symbol::intern("error");
    for attr in attrs {
        if attr.has_name(error) {
            emit_error_attr(cx, attr.span);
        } else if attr.has_name(sym::cfg_attr)
            && let Some(args) = attr.meta_item_list()
        {
            // Symmetric to `check_derive_attrs`: unwrap
            // `#[cfg_attr(pred, error(...))]` so conditional
            // `#[error(...)]` attributes are caught alongside their
            // sibling `#[cfg_attr(pred, derive(thiserror::Error))]`.
            flag_error_in_cfg_attr_payload(cx, args.get(1..).unwrap_or(&[]), error);
        }
    }
}

fn flag_error_in_cfg_attr_payload(cx: &EarlyContext<'_>, items: &[MetaItemInner], error: Symbol) {
    for item in items {
        let Some(meta) = item.meta_item() else {
            continue;
        };
        if meta.has_name(error) {
            emit_error_attr(cx, meta.span);
        } else if meta.has_name(sym::cfg_attr)
            && let MetaItemKind::List(args) = &meta.kind
        {
            flag_error_in_cfg_attr_payload(cx, args.get(1..).unwrap_or(&[]), error);
        }
    }
}

pub(super) fn flag_variant_data_error_attrs(cx: &EarlyContext<'_>, data: &VariantData) {
    for field in data.fields() {
        flag_error_attrs(cx, &field.attrs);
    }
}

pub(super) fn flag_enum_error_attrs(cx: &EarlyContext<'_>, def: &EnumDef) {
    for variant in &def.variants {
        flag_error_attrs(cx, &variant.attrs);
        flag_variant_data_error_attrs(cx, &variant.data);
    }
}

pub(super) fn emit_use(cx: &EarlyContext<'_>, span: Span) {
    // The helper is shared between `use thiserror::...`, `extern
    // crate thiserror`, and the braced top-level `use {...}` form,
    // so the message uses the neutral word "import" rather than
    // saying "`use`".
    span_lint_and_help(
        cx,
        PREFER_DERIVE_MORE_OVER_THISERROR,
        span,
        "`thiserror` import; this catalogue prefers `derive_more::{Display, Error}`",
        None,
        "drop the import and migrate the error type to `#[derive(derive_more::Display, \
         derive_more::Error)]`",
    );
}

pub(super) fn emit_derive(cx: &EarlyContext<'_>, span: Span) {
    span_lint_and_help(
        cx,
        PREFER_DERIVE_MORE_OVER_THISERROR,
        span,
        "error type derived through `thiserror`; this catalogue prefers \
         `derive_more::{Display, Error}`",
        None,
        "replace the derive list and migrate the `#[error(...)]` attributes to `#[display(...)]`",
    );
}

fn emit_error_attr(cx: &EarlyContext<'_>, span: Span) {
    span_lint_and_help(
        cx,
        PREFER_DERIVE_MORE_OVER_THISERROR,
        span,
        "`#[error(...)]` attribute belongs to `thiserror`'s namespace; this catalogue prefers \
         `derive_more::Display`",
        None,
        "rename to `#[display(...)]` and translate positional placeholders (`{0}` -> `{_0}`)",
    );
}
