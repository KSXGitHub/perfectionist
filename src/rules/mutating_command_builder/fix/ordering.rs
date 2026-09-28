//! Whether the change moves a destructor.
//!
//! The rewrite turns a borrow the statement created before its
//! arguments into an owned command created after them, so anything the
//! statement still holds at the end of it changes places with the
//! command. These answer what is still held.

use clippy_utils::ty::needs_ordered_drop;
use clippy_utils::visitors::for_each_expr;
use core::ops::ControlFlow;
use rustc_hir::{Expr, ExprKind, MatchSource, Node, UnOp};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_middle::ty::adjustment::Adjust;
use rustc_span::Ident;

/// Whether evaluating `argument` leaves behind a value whose destructor
/// runs at a point the rewrite would move.
pub(super) fn creates_an_ordered_drop<'tcx>(
    cx: &LateContext<'tcx>,
    argument: &'tcx Expr<'tcx>,
) -> bool {
    for_each_expr(cx.tcx, argument, |expr| {
        // A place is not a new temporary: it is moved or borrowed, and
        // either way the rewrite does not change when it is dropped.
        match outlives_the_call(cx, argument, expr)
            && !expr.is_syntactic_place_expr()
            && needs_ordered_drop(cx, cx.typeck_results().expr_ty(expr))
        {
            true => ControlFlow::Break(()),
            false => ControlFlow::Continue(()),
        }
    })
    .is_some()
}

/// Whether the value `expr` produces is still alive once the call it
/// belongs to has returned.
///
/// A setter takes its argument by value, so a value handed over whole
/// is moved into the call and dropped inside it: `stdin(Stdio::null())`
/// leaves nothing behind, whatever `Stdio`'s destructor does. The walk
/// climbs from `expr` towards the argument it belongs to, and the
/// default at each step is that the value stays: only a hand-over --
/// being passed to a call, or being the receiver of one that takes it
/// by value -- carries it further. A parent the walk does not
/// recognise leaves the value behind, which is the safe way round for
/// a guard whose job is to decline.
///
/// Reading a *part* of a value is the case worth naming, because it
/// looks like a hand-over and is not: only the part moves, and what is
/// left of the temporary is dropped at the end of the statement.
fn outlives_the_call<'tcx>(
    cx: &LateContext<'tcx>,
    argument: &'tcx Expr<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> bool {
    let mut carried = expr;
    loop {
        if is_borrowed(cx, carried) {
            return true;
        }
        if carried.hir_id == argument.hir_id {
            return false;
        }
        let Node::Expr(parent) = cx.tcx.parent_hir_node(carried.hir_id) else {
            return true;
        };
        match parent.kind {
            ExprKind::Field(base, field) if base.hir_id == carried.hir_id => {
                if leaves_a_droppable_sibling(cx, base, field) {
                    return true;
                }
                // Nothing else in the base has a destructor, so what
                // becomes of this field is the whole of the question --
                // and that is the next one up. Answering it here would
                // stop at one level, where a projection can go on.
            }
            ExprKind::Index(base, ..) | ExprKind::Unary(UnOp::Deref, base)
                if base.hir_id == carried.hir_id =>
            {
                return true;
            }
            // `?` moves its payload out of the branch it builds, so
            // the value is handed over as surely as an argument is.
            ExprKind::Match(scrutinee, _, MatchSource::TryDesugar(_))
                if scrutinee.hir_id == carried.hir_id => {}
            ExprKind::Call(_, arguments)
                if arguments
                    .iter()
                    .any(|passed| passed.hir_id == carried.hir_id) => {}
            ExprKind::MethodCall(_, receiver, arguments, _)
                if receiver.hir_id == carried.hir_id
                    || arguments
                        .iter()
                        .any(|passed| passed.hir_id == carried.hir_id) => {}
            _ => return true,
        }
        carried = parent;
    }
}

/// Whether a reference to `expr` is taken, written or inserted.
fn is_borrowed<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> bool {
    matches!(
        cx.tcx.parent_hir_node(expr.hir_id),
        Node::Expr(Expr {
            kind: ExprKind::AddrOf(..),
            ..
        }),
    ) || cx
        .typeck_results()
        .expr_adjustments(expr)
        .iter()
        .any(|adjustment| matches!(adjustment.kind, Adjust::Borrow(_)))
}

/// Whether reading `field` out of `base` leaves anything behind whose
/// destructor runs at the end of the statement, counting only what the
/// projection does not carry away.
///
/// A destructor on the base's own type is the case to watch: it sits on
/// no field, so a scan of the siblings finds nothing -- and a type that
/// has one cannot be taken apart at all, so the field is copied or
/// borrowed and the whole base stays. A base the walk cannot take apart
/// answers `true` as well, since a guard that cannot tell should
/// decline.
fn leaves_a_droppable_sibling<'tcx>(
    cx: &LateContext<'tcx>,
    base: &'tcx Expr<'tcx>,
    field: Ident,
) -> bool {
    let base_ty = cx.typeck_results().expr_ty(base);
    match base_ty.kind() {
        ty::Adt(adt, args) if adt.is_struct() => {
            (adt.has_dtor(cx.tcx) && needs_ordered_drop(cx, base_ty))
                || adt.non_enum_variant().fields.iter().any(|sibling| {
                    sibling.name != field.name
                        && needs_ordered_drop(cx, sibling.ty(cx.tcx, args).skip_norm_wip())
                })
        }
        // A tuple names its fields by position and carries their types
        // directly rather than through an `AdtDef`.
        ty::Tuple(elements) => {
            let taken = field.name.as_str().parse::<usize>().ok();
            elements
                .iter()
                .enumerate()
                .any(|(index, element)| Some(index) != taken && needs_ordered_drop(cx, element))
        }
        _ => true,
    }
}

/// Whether consuming `receiver` leaves part of a temporary behind for
/// the statement to drop.
///
/// Moving a field out of a temporary leaves its other fields to be
/// dropped at the end of the statement, where the owned command the
/// rewrite produces is created later still and so drops first. A
/// temporary with nothing else to drop is unaffected, which is the
/// ordinary `make().command`.
pub(super) fn leaves_a_sibling_behind<'tcx>(
    cx: &LateContext<'tcx>,
    receiver: &'tcx Expr<'tcx>,
) -> bool {
    let mut carried = receiver;
    while let ExprKind::Field(base, field) = carried.kind {
        if leaves_a_droppable_sibling(cx, base, field) {
            return true;
        }
        carried = base;
    }
    false
}
