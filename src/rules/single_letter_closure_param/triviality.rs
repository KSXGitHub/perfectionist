//! Classifiers for the "trivial single-expression callback"
//! exemption.
//!
//! A closure is exempted if either:
//! - its body is a single expression and the enclosing call is one of
//!   the configured comparison / fold callees, or
//! - its body is a single expression that is a trivial wrapper around
//!   one of the closure's parameters (field access, method call, one-
//!   argument call, reference).
//!
//! The helpers here decide each predicate. The driver in
//! [`super`] composes them.

use rustc_hir as hir;
use rustc_hir::def::Res;
use rustc_lint::LateContext;
use rustc_span::Symbol;

use crate::common::binding_hir_id;

/// If `body.value` is a single expression — either directly or
/// wrapped in a block with no statements — return that
/// expression. Otherwise return `None`.
pub(super) fn single_expression_body<'hir>(
    body: &'hir hir::Body<'hir>,
) -> Option<&'hir hir::Expr<'hir>> {
    let value = body.value;
    if let hir::ExprKind::Block(block, _) = value.kind {
        if !block.stmts.is_empty() {
            return None;
        }
        block.expr
    } else {
        Some(value)
    }
}

/// Return the callee name of the parent call expression of
/// `closure_expr`, if any. `MethodCall` returns the method's final
/// segment; `Call` returns the final segment of the callee path.
/// Anything else (`if`, indexing, …) returns `None`.
pub(super) fn parent_call_callee_name<'tcx>(
    lint_context: &LateContext<'tcx>,
    closure_expr: &'tcx hir::Expr<'tcx>,
) -> Option<Symbol> {
    let parent = lint_context.tcx.parent_hir_node(closure_expr.hir_id);
    let hir::Node::Expr(parent_expr) = parent else {
        return None;
    };
    match parent_expr.kind {
        hir::ExprKind::MethodCall(segment, _, _, _) => Some(segment.ident.name),
        hir::ExprKind::Call(callee, _) => path_final_segment(callee),
        _ => None,
    }
}

fn path_final_segment<'hir>(expr: &'hir hir::Expr<'hir>) -> Option<Symbol> {
    let hir::ExprKind::Path(qpath) = &expr.kind else {
        return None;
    };
    let segment = match qpath {
        hir::QPath::Resolved(_, path) => path.segments.last()?,
        hir::QPath::TypeRelative(_, segment) => *segment,
    };
    Some(segment.ident.name)
}

/// Returns whether `expr` is a "trivial wrapper" around one of
/// the closure's parameters:
/// - a field access `param.field`,
/// - a method call `param.foo(args)`,
/// - a one-argument call `f(param)`,
/// - a reference `&param`.
///
/// In every shape, `*` / `&` operators around the reference to
/// the parameter are peeled by `is_param_ref` before matching,
/// so `|s| (*s).foo()` and `|s| f(&*s)` both qualify.
pub(super) fn is_trivial_wrapper<'hir>(
    expr: &'hir hir::Expr<'hir>,
    params: &'hir [hir::Param<'hir>],
) -> bool {
    match expr.kind {
        hir::ExprKind::Field(receiver, _) => is_param_ref(receiver, params),
        hir::ExprKind::MethodCall(_, receiver, _, _) => is_param_ref(receiver, params),
        hir::ExprKind::Call(_, args) => args.len() == 1 && is_param_ref(&args[0], params),
        hir::ExprKind::AddrOf(_, _, inner) => is_param_ref(inner, params),
        _ => false,
    }
}

/// "Refers to a parameter, possibly through one or more `*` /
/// `&` operators." Peeling through these keeps `|s| (*s).foo()`
/// classified as a trivial wrapper, since the deref is a
/// purely-structural step the reader does not need help with.
fn is_param_ref(expr: &hir::Expr<'_>, params: &[hir::Param<'_>]) -> bool {
    let mut expr = expr;
    loop {
        match &expr.kind {
            hir::ExprKind::Unary(hir::UnOp::Deref, inner) | hir::ExprKind::AddrOf(_, _, inner) => {
                expr = inner
            }
            hir::ExprKind::Path(hir::QPath::Resolved(None, path)) => {
                let Res::Local(local_hir_id) = path.res else {
                    return false;
                };
                return params
                    .iter()
                    .any(|param| binding_hir_id(param.pat) == Some(local_hir_id));
            }
            _ => return false,
        }
    }
}
