//! Deciding whether a body hands over a borrow of `self`'s own data
//! without running anything on the way.
//!
//! The question is *cost*, and the written form answers only half of
//! it. `&self.name` reaching a `&str` return derefs the `String`, and a
//! `Deref` is a call: free where the implementation hands back a
//! pointer the value already holds, not free where it runs an
//! initializer. Neither spelling shows which, because a deref coercion
//! is not an expression — it is an adjustment typeck records against
//! the expression that needed it.
//!
//! So the types whose `Deref` only projects are named, and an
//! unrecognised one counts as code. That direction is the safe one for
//! this rule: a free borrow reached through a wrapper this module
//! cannot name is missed, where the other direction would tell the
//! author of a `LazyLock` field to rename a method the API guidelines
//! say is right.
//!
//! What the *callee* of an `as_*` call costs is not read at all. The
//! guidelines give that prefix the same meaning this rule enforces, so
//! trusting it here is trusting what the catalogue already holds
//! authors to.

use rustc_hir::{BorrowKind, Expr, ExprKind, QPath, UnOp};
use rustc_lint::LateContext;
use rustc_middle::ty::adjustment::{Adjust, DerefAdjustKind};
use rustc_middle::ty::{self, Ty, TypeckResults};
use rustc_span::{Symbol, kw};

/// The prefix a method call in the body may be trusted to be free under,
/// and the one a flagged method should have worn instead: the guidelines
/// give both the same meaning.
pub(super) const AS_PREFIX: &str = "as_";

/// The types whose `Deref` hands back a pointer the value already
/// holds. `String` and `Box` are lang items and are asked for
/// separately.
const PROJECTING_DEREFS: &[&str] = &["Vec", "PathBuf", "OsString", "cstring_type", "Rc", "Arc"];

/// Whether the body hands over a borrow it already had: a place inside
/// `self`, or a single `as_*` call on one. Anything else is left alone,
/// a free body among them, since what an arbitrary body costs is not a
/// question this can answer.
pub(super) fn costless_body<'tcx>(
    cx: &LateContext<'tcx>,
    typeck: &TypeckResults<'tcx>,
    expr: &Expr<'_>,
) -> bool {
    if let ExprKind::MethodCall(segment, receiver, [], _) = expr.kind {
        return segment.ident.name.as_str().starts_with(AS_PREFIX)
            && borrows_self(cx, typeck, receiver);
    }
    borrows_self(cx, typeck, expr)
}

/// Whether `expr` names a place inside `self`, or borrows one, reaching
/// it without running a `Deref` body that does more than project:
/// `self.name`, `&self.name`, `&self.inner.name`, `&*self.boxed`.
fn borrows_self<'tcx>(
    cx: &LateContext<'tcx>,
    typeck: &TypeckResults<'tcx>,
    mut expr: &Expr<'_>,
) -> bool {
    loop {
        // The coercions applied to this expression, in order, each one
        // starting from what the last produced.
        let mut source = typeck.expr_ty(expr);
        for adjustment in typeck.expr_adjustments(expr) {
            if let Adjust::Deref(DerefAdjustKind::Overloaded(_)) = adjustment.kind
                && !projects_a_pointer(cx, source)
            {
                return false;
            }
            source = adjustment.target;
        }
        expr = match expr.kind {
            ExprKind::AddrOf(BorrowKind::Ref, _, inner) => inner,
            ExprKind::Field(base, _) => base,
            // An explicit `*x` on a type with an overloaded `Deref` is
            // recorded as a method call on the unary expression itself,
            // where the loop above finds no adjustment to read.
            ExprKind::Unary(UnOp::Deref, base) => {
                if typeck.is_method_call(expr) && !projects_a_pointer(cx, typeck.expr_ty(base)) {
                    return false;
                }
                base
            }
            ExprKind::Path(QPath::Resolved(None, path)) => {
                return matches!(path.segments, [segment] if segment.ident.name == kw::SelfLower);
            }
            _ => return false,
        };
    }
}

/// Whether dereferencing `ty` only projects a pointer it already holds.
fn projects_a_pointer<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    let ty::Adt(adt, _) = ty.peel_refs().kind() else {
        return false;
    };
    let did = adt.did();
    if Some(did) == cx.tcx.lang_items().string() || Some(did) == cx.tcx.lang_items().owned_box() {
        return true;
    }
    PROJECTING_DEREFS
        .iter()
        .any(|name| cx.tcx.is_diagnostic_item(Symbol::intern(name), did))
}
