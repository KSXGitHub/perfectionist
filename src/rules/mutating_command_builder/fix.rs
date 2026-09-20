//! Building the rewrite the fixer applies.
//!
//! A chain is rewritten whole or not at all, and that is the whole of
//! why it is safe. Each later link took the `&mut Command` the previous
//! one returned, so renaming only the head leaves the rest calling
//! std's setters against a receiver that is now owned -- which is how a
//! by-value method of the author's own comes to be found before the
//! inherent one, compiling, with nothing in the diff to show it.
//! Renaming every link takes that name out of play.
//!
//! The chain's own value still has to land somewhere. Where a statement
//! discards it, nothing constrains it. Where something reads it,
//! prefixing `&mut ` gives back the type the original had. Where a
//! further call takes it as a receiver, that call autorefs from the
//! owned command, which is the form a person writes by hand.
//!
//! One thing that leaves: the trailing call's receiver is an owned
//! `Command` where it was a `&mut Command`, so it resolves to the same
//! method unless the author has an in-scope by-value method of that
//! name for `Command`. `CommandExtra` declares none -- it is `with_*`
//! and `without_*` throughout -- so reaching that needs deliberately
//! shadowing one of `Command`'s own methods.

use super::setter::{self, Conversion};
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
    // The counterpart has to exist where the call is written, and
    // moving the receiver takes nothing away from anyone only where
    // nobody else holds it.
    if !inputs.trait_is_imported || !inputs.receiver_is_a_temporary {
        return None;
    }
    let mut parts = link(
        cx,
        call,
        inputs.by_value_form,
        inputs.conversion,
        inputs.names_generic_arguments,
    )?;
    // Follow the setters this one feeds. Any of them that cannot be
    // rewritten takes the whole chain with it, because a half-rewritten
    // chain is the shape that resolves somewhere new.
    let mut tail = call;
    while let Node::Expr(parent) = cx.tcx.parent_hir_node(tail.hir_id) {
        let ExprKind::MethodCall(method, receiver, arguments, _) = parent.kind else {
            break;
        };
        if receiver.hir_id != tail.hir_id {
            break;
        }
        let Some(by_value_form) = setter::by_value_form(method.ident.name) else {
            // Not a setter, so it ends the chain and takes the owned
            // command by autoref, as a hand-written call would.
            return Some(parts);
        };
        // A later link resolving anywhere but to `Command`'s own setter
        // is already trait-mediated, and renaming it would move it.
        if !setter::resolves_to_an_inherent_command_method(cx, parent) {
            return None;
        }
        parts.extend(link(
            cx,
            parent,
            by_value_form,
            setter::argument_conversion(cx, parent, arguments),
            method.args.is_some_and(|written| !written.args.is_empty()),
        )?);
        tail = parent;
    }
    // Nothing further calls it, so the chain's own value is what has to
    // keep its type. The prefix goes in front of the *tail*, not the
    // flagged head: a chain's span usually starts at its head, but a
    // macro invocation wrapping the head alone extends the tail's span
    // to the left of it, and `&mut ` inside the invocation borrows only
    // the part the macro was handed.
    if position(cx, tail)? == Position::TypeIsKept {
        parts.push((tail.span.shrink_to_lo(), "&mut ".to_owned()));
    }
    Some(parts)
}

/// The edits one link of the chain needs, or `None` where it cannot be
/// rewritten at all.
fn link<'tcx>(
    cx: &LateContext<'tcx>,
    call: &'tcx Expr<'tcx>,
    by_value_form: &'static str,
    conversion: Conversion,
    names_generic_arguments: bool,
) -> Option<Vec<(Span, String)>> {
    let ExprKind::MethodCall(method, _, arguments, _) = call.kind else {
        return None;
    };
    // A written turbofish names the std setter's generic parameters,
    // and the counterpart's do not correspond to them one for one --
    // only `with_envs` takes the same set. Transferring them would be a
    // guess, and dropping them leans on inference, so neither is a
    // rewrite this can promise compiles.
    if names_generic_arguments {
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
    if call.span.from_expansion() || method.ident.span.from_expansion() {
        return None;
    }
    let mut edits = vec![(method.ident.span, by_value_form.to_owned())];
    if conversion == Conversion::IntoNeeded {
        let argument = arguments.first()?;
        // An argument a macro produced is written in the macro's body,
        // so the conversion would be appended there: to every other
        // expansion of it at once, and to a file the fixer may not
        // even be rewriting. The call's own span says nothing about
        // its arguments', so this is asked separately.
        if argument.span.from_expansion() {
            return None;
        }
        edits.push((
            argument.span,
            format!("{}.into()", Sugg::hir(cx, argument, "..").maybe_paren()),
        ));
    }
    Some(edits)
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
