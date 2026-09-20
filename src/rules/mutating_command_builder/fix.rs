//! Building the rewrite the fixer may apply.
//!
//! The rename alone changes the expression's type from `&mut Command`
//! to `Command`, and that is what made the change look undecidable: the
//! context may have wanted the borrow, and a later call in the same
//! chain can resolve differently against an owned receiver, compiling
//! all the while. Prefixing `&mut ` restores the type, and a
//! type-preserving rewrite is accepted wherever the original was, by
//! the context and by method resolution alike.
//!
//! So the rewrite is not one edit but up to three: the rename, an
//! `.into()` where the counterpart takes by value what the setter took
//! generically, and the `&mut `. What is left after that is checkable,
//! and this module checks it.

use super::setter::Conversion;
use clippy_utils::sugg::Sugg;
use clippy_utils::ty::needs_ordered_drop;
use clippy_utils::visitors::for_each_expr;
use core::ops::ControlFlow;
use rustc_hir::{Expr, ExprKind, Node, StmtKind};
use rustc_lint::LateContext;
use rustc_span::Span;

/// What the caller has already decided about the call.
pub(super) struct Inputs {
    pub(super) by_value_form: &'static str,
    pub(super) conversion: Conversion,
    pub(super) receiver_is_a_temporary: bool,
    pub(super) trait_is_imported: bool,
    pub(super) names_generic_arguments: bool,
}

/// The spans to replace, where every part of the change is known and
/// the result compiles. `None` where any part is not.
pub(super) fn parts<'tcx>(
    cx: &LateContext<'tcx>,
    call: &'tcx Expr<'tcx>,
    inputs: &Inputs,
) -> Option<Vec<(Span, String)>> {
    let ExprKind::MethodCall(method, _, arguments, _) = call.kind else {
        return None;
    };
    // The counterpart has to exist where the call is written.
    if !inputs.trait_is_imported {
        return None;
    }
    // Moving the receiver takes nothing away from anyone only where
    // nobody else holds it.
    if !inputs.receiver_is_a_temporary {
        return None;
    }
    // A written turbofish names the std setter's generic parameters,
    // and the counterpart's do not correspond to them one for one --
    // only `with_envs` takes the same set. Transferring them would be a
    // guess, and dropping them leans on inference, so neither is a
    // rewrite this can promise compiles.
    if inputs.names_generic_arguments {
        return None;
    }
    // The owned command is created after the arguments are evaluated,
    // where the borrow it replaces was created before them, so it
    // becomes the statement's last temporary and drops first. Only an
    // argument's own temporary is positioned to observe that.
    if arguments
        .iter()
        .any(|argument| creates_an_ordered_drop(cx, argument))
    {
        return None;
    }
    let position = position(cx, call)?;
    let mut parts = vec![(method.ident.span, inputs.by_value_form.to_owned())];
    if inputs.conversion == Conversion::IntoNeeded {
        let argument = arguments.first()?;
        parts.push((
            argument.span,
            format!("{}.into()", Sugg::hir(cx, argument, "..").maybe_paren()),
        ));
    }
    if position == Position::TypeIsKept {
        parts.push((call.span.shrink_to_lo(), "&mut ".to_owned()));
    }
    Some(parts)
}

/// Where the call's value lands, in the terms the rewrite cares about.
#[derive(PartialEq, Eq)]
enum Position {
    /// A statement discards it, so nothing constrains the type and the
    /// rename stands alone.
    Discarded,
    /// Something reads it, so the rewrite has to hand back the same
    /// `&mut Command` the original did.
    TypeIsKept,
}

/// `None` where the rewrite cannot be written at this position, rather
/// than where it would not compile.
fn position(cx: &LateContext<'_>, call: &Expr<'_>) -> Option<Position> {
    if call.span.from_expansion() {
        return None;
    }
    match cx.tcx.parent_hir_node(call.hir_id) {
        Node::Stmt(statement) => match statement.kind {
            StmtKind::Semi(_) if !statement.span.from_expansion() => Some(Position::Discarded),
            _ => None,
        },
        // A macro holding the caller's expression puts every use of it
        // at the one span this would edit. The type is kept at each of
        // them, so they would all still compile -- but the rule cannot
        // see what else the body does with the tokens, and `stringify!`
        // is enough to make type-preserving stop meaning
        // behaviour-preserving.
        Node::Expr(parent) if parent.span.from_expansion() => None,
        // `&mut` binds looser than these, so the prefix would have to
        // carry parentheses -- and `(&mut command).arg(..)` reads worse
        // than the call it replaces, which is the whole point of the
        // rule. The diagnostic shows the bare rename there instead.
        Node::Expr(Expr {
            kind: ExprKind::MethodCall(_, receiver, ..),
            ..
        }) if receiver.hir_id == call.hir_id => None,
        Node::Expr(Expr {
            kind: ExprKind::Field(base, _) | ExprKind::Index(base, ..),
            ..
        }) if base.hir_id == call.hir_id => None,
        Node::Expr(_) => Some(Position::TypeIsKept),
        _ => None,
    }
}

/// Whether evaluating `argument` builds a value whose destructor runs
/// at a point the rewrite would move.
fn creates_an_ordered_drop<'tcx>(cx: &LateContext<'tcx>, argument: &'tcx Expr<'tcx>) -> bool {
    for_each_expr(cx.tcx, argument, |expr| {
        // A place is not a new temporary: it is moved or borrowed, and
        // either way the rewrite does not change when it is dropped.
        match !expr.is_syntactic_place_expr()
            && needs_ordered_drop(cx, cx.typeck_results().expr_ty(expr))
        {
            true => ControlFlow::Break(()),
            false => ControlFlow::Continue(()),
        }
    })
    .is_some()
}
