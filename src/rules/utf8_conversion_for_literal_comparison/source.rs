//! The converted value a comparison names, and how the comparison
//! reaches the conversion.
//!
//! A comparison may apply the conversion itself, or reach it through
//! an `Option` adapter, as `path.file_name().and_then(|name|
//! name.to_str())` does. Both are local shapes: the adapter's closure
//! sits in the expression being read, so neither needs the value
//! traced from a binding.

use crate::utf8_conversion::{Conversion, conversion};
use rustc_hir::{Body, Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::sym;

/// How a comparison reaches the conversion it names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Reached {
    /// The comparison applied the conversion to the value itself, so
    /// the rewrite names that value.
    Directly,
    /// The comparison applied the conversion inside the closure of an
    /// `Option::map` or `Option::and_then`, so the rewrite names the
    /// `Option` and keeps the `Some` the comparison already carries.
    ThroughOption,
}

/// A conversion a comparison names, with the value the rewrite should
/// name in its place.
pub(super) struct Converted<'tcx> {
    /// The conversion found, whose `source` is the value the rewrite
    /// names: the converted value itself for [`Reached::Directly`], or
    /// the adapter's receiver for [`Reached::ThroughOption`].
    pub(super) conversion: Conversion<'tcx>,
    pub(super) reached: Reached,
}

/// The conversion `expr` names, where it names one.
pub(super) fn converted<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<Converted<'tcx>> {
    if let Some(conversion) = conversion(cx, expr) {
        return Some(Converted {
            conversion,
            reached: Reached::Directly,
        });
    }
    through_option(cx, expr)
}

/// The conversion an `Option::map` or `Option::and_then` applies in its
/// closure, with the adapter's receiver standing in as the value the
/// rewrite names.
///
/// The closure has to convert its own parameter and nothing else. A
/// closure reaching for anything from its surroundings is computing
/// something the comparison's literal does not describe, so the
/// rewrite would not be the same question asked of bytes.
fn through_option<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> Option<Converted<'tcx>> {
    let ExprKind::MethodCall(segment, receiver, [adapter], _) = expr.kind else {
        return None;
    };
    if !matches!(segment.ident.name.as_str(), "map" | "and_then") {
        return None;
    }
    // Only an `Option` receiver, since the comparison's wrapper is a
    // `Some`. A `Result` reaches the same text through the same
    // adapters, and its `Err` carries a payload the rewrite would have
    // to preserve, so it is left out of this tier.
    let ty::Adt(option, _) = cx.typeck_results().expr_ty(receiver).peel_refs().kind() else {
        return None;
    };
    if !cx.tcx.is_diagnostic_item(sym::Option, option.did()) {
        return None;
    }
    let ExprKind::Closure(closure) = adapter.kind else {
        return None;
    };
    let body = cx.tcx.hir_body(closure.body);
    let conversion = conversion(cx, crate::common::unwrap_block(body.value))?;
    converts_own_parameter(body, &conversion).then_some(Converted {
        conversion: Conversion {
            source: receiver,
            ..conversion
        },
        reached: Reached::ThroughOption,
    })
}

/// Whether the [`Conversion`] converted the closure's own parameter.
fn converts_own_parameter(body: &Body<'_>, conversion: &Conversion<'_>) -> bool {
    let [parameter] = body.params else {
        return false;
    };
    let Some(bound) = crate::common::binding_hir_id(parameter.pat) else {
        return false;
    };
    let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = conversion.source.kind else {
        return false;
    };
    matches!(path.res, rustc_hir::def::Res::Local(local) if local == bound)
}
