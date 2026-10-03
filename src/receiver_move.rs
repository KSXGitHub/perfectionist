//! Whether a lifted adapter may take the receiver the folded one has.
//!
//! Every rule here lifts work into an adapter placed in front of the one
//! it came from, and the three adapters they lift into -- `map`, `filter`
//! and `take_while` -- all take the iterator by value. The adapter the
//! work came from need not: `Iterator::any`, `find`, `rfind`, `position`
//! and `find_map` take `&mut self`, so the folded form leaves the
//! receiver where it was and the split moves it away.
//!
//! That is only a problem where the receiver is something a move cannot
//! be taken from, so the question is asked of the method first and of
//! the receiver second.

use crate::binding_uses::uses;
use rustc_hir::def::Res;
use rustc_hir::{Expr, ExprKind, QPath, UnOp};
use rustc_lint::LateContext;

/// Whether the lifted adapter can be given `receiver`, where `call` is
/// the folded adapter's own call.
pub(crate) fn movable<'tcx>(
    cx: &LateContext<'tcx>,
    call: &Expr<'tcx>,
    receiver: &'tcx Expr<'tcx>,
) -> bool {
    if !borrows_the_receiver(cx, call) {
        return true;
    }
    match receiver.kind {
        // A local is movable where this call is the only thing naming it:
        // anything else naming it is `E0382` once the split moves it, and
        // whether that other mention runs first is a question a span
        // cannot answer.
        ExprKind::Path(QPath::Resolved(None, path)) => match path.res {
            Res::Local(local) => named_once(cx, local),
            _ => false,
        },
        // A place the receiver does not own: moving out of one is
        // `E0507`, and a `Copy` iterator is rare enough to decline with
        // the rest.
        ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Unary(UnOp::Deref, _) => false,
        // Everything else is a value the folded form made, which nothing
        // else can be holding.
        _ => true,
    }
}

/// Whether the method `call` resolves to takes its receiver by
/// reference, which is what leaves the receiver usable after it.
pub(crate) fn borrows_the_receiver<'tcx>(cx: &LateContext<'tcx>, call: &Expr<'tcx>) -> bool {
    let Some(method) = cx.typeck_results().type_dependent_def_id(call.hir_id) else {
        return false;
    };
    cx.tcx
        .fn_sig(method)
        .skip_binder()
        .skip_binder()
        .inputs()
        .first()
        .is_some_and(|receiver| receiver.is_ref())
}

/// Whether the enclosing body names `local` exactly once.
fn named_once(cx: &LateContext<'_>, local: rustc_hir::HirId) -> bool {
    let Some(body) = cx.enclosing_body else {
        return false;
    };
    uses(cx, cx.tcx.hir_body(body).value, &[local]).len() == 1
}
