//! What the fold's initial value has to be to become the plural's
//! receiver.
//!
//! The suggestion turns the initial value from the fold's argument into
//! a method-call receiver. An argument can take its type, and its
//! bracketing, from the call around it; a receiver has to bring both
//! along itself.

use clippy_utils::higher::Range;
use clippy_utils::ty::expr_type_is_certain;
use clippy_utils::visitors::for_each_expr_without_closures;
use core::ops::ControlFlow;
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::{Block, Expr, ExprKind, LetStmt, MatchSource, Node, Path, QPath};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, TypeVisitableExt};

/// Whether a struct literal appears anywhere in `expr`.
///
/// Unbracketed, one at the head of a method chain reads as the start of
/// a block where the call lands in a scrutinee or a condition. Anywhere
/// else the brackets are redundant, and rustc does not warn about them.
/// A range is a struct literal to HIR but not to the parser, so it is
/// skipped.
pub(super) fn holds_a_struct_literal<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> bool {
    for_each_expr_without_closures(expr, |expr| {
        if matches!(expr.kind, ExprKind::Struct(..)) && Range::hir(cx, expr).is_none() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    })
    .is_some()
}

/// Whether `expr` fixes its own type, rather than taking it from the
/// fold it is passed to.
///
/// Method resolution needs a receiver's type before it reads the call,
/// so a receiver whose type only the fold supplied has none.
/// `start.into()` folded with `Command::without_env` converts into a
/// `Command` because the folder says so; as
/// `start.into().without_envs(..)` it converts into nothing in
/// particular. Measured as a machine-applicable `E0282`.
///
/// A call whose declared return type names no type parameter fixes it,
/// whatever its arguments: `Command::new(format!(..))` is a `Command`. A
/// binding declared without a type is asked about its initializer, a
/// block about its tail, `?` and `.await` about their operand, and a
/// branching expression about its branches, any one of which fixes the
/// type of the whole. Everything else, other bindings included, is put
/// to Clippy's `expr_type_is_certain`, which errs toward no: a closure
/// parameter or a pattern binding can take its type from the fold as
/// well.
pub(super) fn fixes_its_own_type<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> bool {
    let typeck = cx.typeck_results();
    // An expression that diverges fixes no type, though `todo!()` is a
    // call to a path Clippy counts as certain. In a branch beside one
    // whose type the fold supplies, it leaves the whole untyped.
    if typeck.expr_ty(expr).is_never() {
        return false;
    }
    let callee = match expr.kind {
        ExprKind::Call(callee, _) => match typeck.expr_ty(callee).kind() {
            ty::FnDef(def_id, _) => Some(*def_id),
            _ => None,
        },
        ExprKind::MethodCall(..) => typeck.type_dependent_def_id(expr.hir_id),
        ExprKind::Path(QPath::Resolved(
            None,
            Path {
                res: Res::Local(local),
                ..
            },
        )) => {
            if let Node::LetStmt(LetStmt {
                ty: None,
                init: Some(init),
                ..
            }) = cx.tcx.parent_hir_node(*local)
            {
                return fixes_its_own_type(cx, init);
            }
            None
        }
        ExprKind::Block(
            Block {
                expr: Some(tail), ..
            },
            _,
        ) => return fixes_its_own_type(cx, tail),
        ExprKind::If(_, then, Some(otherwise)) => {
            return fixes_its_own_type(cx, then) || fixes_its_own_type(cx, otherwise);
        }
        ExprKind::Match(scrutinee, _, MatchSource::TryDesugar(_) | MatchSource::AwaitDesugar) => {
            if let ExprKind::Call(_, [operand]) = scrutinee.kind {
                return fixes_its_own_type(cx, operand);
            }
            None
        }
        ExprKind::Match(_, arms, _) => {
            return arms.iter().any(|arm| fixes_its_own_type(cx, arm.body));
        }
        _ => None,
    };
    callee.is_some_and(|callee| returns_a_concrete_type(cx, callee))
        || expr_type_is_certain(cx, expr)
}

/// Whether the declared return type of `callee` names no type or const
/// parameter, its own or its parent's.
fn returns_a_concrete_type(cx: &LateContext<'_>, callee: DefId) -> bool {
    !cx.tcx
        .fn_sig(callee)
        .skip_binder()
        .output()
        .skip_binder()
        .has_non_region_param()
}
