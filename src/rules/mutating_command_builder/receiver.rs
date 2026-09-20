//! Whether the by-value form could take the receiver the setter was
//! called on.
//!
//! The receiver's *type* does not answer this, which is what makes the
//! walk here worth its own module: a `Command` field reached through
//! `&mut self` has type `Command` with no reference in sight, and
//! moving out of it is `E0507`. So the place expression is walked
//! instead, and only what the caller owns outright is accepted.

use clippy_utils::ty::has_drop;
use rustc_hir::def::Res;
use rustc_hir::{Expr, ExprKind, QPath};
use rustc_lint::LateContext;
use rustc_middle::ty::adjustment::Adjust;

/// Whether the by-value form could consume `receiver`.
///
/// Anything unrecognised is treated as not consumable, which costs a
/// missed diagnostic rather than an unfixable one.
pub(super) fn can_be_consumed(cx: &LateContext<'_>, receiver: &Expr<'_>) -> bool {
    match receiver.kind {
        // A local binding, `self` taken by value among them -- but not
        // one belonging to an enclosing body, which is an upvar the
        // closure only borrows. Moving out of that is `E0507`, and
        // making the closure take it by value turns an `FnMut` into an
        // `FnOnce`.
        ExprKind::Path(QPath::Resolved(None, path)) => match path.res {
            Res::Local(local) => {
                cx.tcx.hir_enclosing_body_owner(local)
                    == cx.tcx.hir_enclosing_body_owner(receiver.hir_id)
            }
            _ => false,
        },
        // Never an index, for the reason `produces_a_temporary` gives.
        ExprKind::Index(..) => false,
        // A field of something the caller owns, so long as reaching it
        // does not pass through a reference -- which is what an
        // autoderef adjustment on the base records -- and so long as
        // the base does not implement `Drop`, since moving a field out
        // of such a value is `E0509` with no way to finish the fix.
        ExprKind::Field(base, _) => {
            !cx.typeck_results()
                .expr_adjustments(base)
                .iter()
                .any(|adjustment| matches!(adjustment.kind, Adjust::Deref(_)))
                && !has_drop(cx, cx.typeck_results().expr_ty(base))
                && can_be_consumed(cx, base)
        }
        // A value this expression produced, which is a temporary
        // nobody else holds a claim on.
        _ => produces_a_temporary(receiver),
    }
}

/// Whether `receiver` is a value the expression produced rather than a
/// place the surrounding code still holds.
///
/// Consuming a temporary takes nothing away from anyone. Consuming a
/// binding or a field moves it, which is `E0382` where later code reads
/// it and `E0507` where a closure captured it -- so the rename is only
/// rendered on a temporary, even though the *diagnostic* is right on a
/// binding too.
pub(super) fn produces_a_temporary(receiver: &Expr<'_>) -> bool {
    match receiver.kind {
        // `is_syntactic_place_expr` answers true for any field
        // whatever its base, so recurse: a field of a temporary is a
        // temporary, and only the base decides.
        ExprKind::Field(base, _) => produces_a_temporary(base),
        // An index is never consumable, whatever its base: `Index`
        // hands back a borrow (`E0507`), and an array index moves out
        // of a non-copy array (`E0508`).
        ExprKind::Index(..) => false,
        _ => !receiver.is_syntactic_place_expr(),
    }
}
