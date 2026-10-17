//! The call shape the two defaulted-mapper rules read.
//!
//! `Option::map_or`, `Option::map_or_else`, `Result::map_or` and
//! `Result::map_or_else` each take a default and a mapper, in that
//! order. `map_or` and `map_or_else` fire on one method
//! name apiece, and what they share is finding the call: the name, the
//! receiver's family, the default, and the span to report at.
//!
//! Which remedy each one names is its own business. The predicates an
//! `Option` answers with are not the ones a `Result` answers with, and
//! only a default that is a value has one at all.

use crate::common::hir_in_external_macro;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_span::{Span, sym};

/// Which of the two families the receiver belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    Option,
    Result,
}

/// A call to a defaulted mapper, in the parts a diagnostic needs.
pub(crate) struct DefaultedCall<'tcx> {
    pub(crate) family: Family,
    /// The first argument: the default, standing ahead of the mapper it
    /// is a default for.
    pub(crate) default: &'tcx Expr<'tcx>,
    /// The method name and its arguments, without the receiver or the
    /// dot — `map_or(0, f)` out of `value.map_or(0, f)`. A receiver can
    /// run to several lines and is not what the diagnostic is about.
    pub(crate) span: Span,
}

/// The call `expr` makes to `method` on an `Option` or a `Result`,
/// where the author wrote it themselves.
pub(crate) fn defaulted_call<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    method: &str,
) -> Option<DefaultedCall<'tcx>> {
    let ExprKind::MethodCall(segment, receiver, [default, _mapper], span) = expr.kind else {
        return None;
    };
    if segment.ident.name.as_str() != method {
        return None;
    }
    // A call an expansion produced belongs to whoever wrote the macro,
    // and the report would name a line its reader cannot act on, once
    // per expansion site. `report_in_external_macro: false` covers
    // another crate's macro; a `macro_rules!` in the crate being linted
    // needs this. A call written as a macro *argument* keeps the
    // author's own span, so `assert!(opt.map_or(false, f))` is reported.
    if expr.span.from_expansion() {
        return None;
    }
    // The inherent methods and no others. An extension trait can lend
    // the same name to a receiver of the same family — a `&Option<T>`
    // whose payload cannot be moved out resolves to the trait's method,
    // since the inherent one takes `self` — and that call is not this
    // one.
    let method_id = cx.typeck_results().type_dependent_def_id(expr.hir_id)?;
    if cx.tcx.trait_of_assoc(method_id).is_some() {
        return None;
    }
    let family = family(cx, receiver)?;
    // A derive can stamp a user-source span on a call it synthesised,
    // which leaves the expansion check above nothing to find, and the
    // diagnostic span this rule picks is narrower than the call, so
    // `report_in_external_macro: false` does not see the expansion
    // either.
    if hir_in_external_macro(cx, expr.hir_id, span) {
        return None;
    }
    Some(DefaultedCall {
        family,
        default,
        span,
    })
}

/// Which family `receiver`'s type belongs to, if either. The type is
/// taken as adjusted and peeled of references, the way the method call
/// itself reached the receiver: `map_or` takes `self`, so a
/// `&Option<u8>` arrives derefed.
fn family(cx: &LateContext<'_>, receiver: &Expr<'_>) -> Option<Family> {
    let adt = cx
        .typeck_results()
        .expr_ty_adjusted(receiver)
        .peel_refs()
        .ty_adt_def()?;
    let carries = |name| cx.tcx.is_diagnostic_item(name, adt.did());
    if carries(sym::Option) {
        return Some(Family::Option);
    }
    carries(sym::Result).then_some(Family::Result)
}
