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
//! The trailing call is the one name the rewrite cannot change, and its
//! receiver becomes an owned `Command` where it was a `&mut Command`. A
//! by-value method of that name in scope for `Command` is then found
//! before the inherent one, compiling and calling something else, so
//! the chain is declined wherever the traits in scope supply one.
//!
//! The names the rewrite does introduce are safe for a different
//! reason. `CommandExtra` has to be imported before a rewrite is built
//! at all, so a competing `with_*` in scope for `Command` leaves two
//! applicable candidates and the renamed call is `E0034` -- an error
//! the fixer reverts, rather than something that quietly resolves
//! elsewhere.

use super::setter::{self, Conversion};
use clippy_utils::sugg::Sugg;
use clippy_utils::ty::needs_ordered_drop;
use clippy_utils::visitors::for_each_expr;
use core::ops::ControlFlow;
use rustc_hir::{Expr, ExprKind, Node, StmtKind};
use rustc_lint::LateContext;
use rustc_middle::ty::AssocTag;
use rustc_span::def_id::DefId;
use rustc_span::{Span, Symbol};

/// What the caller has already decided about the call.
pub(super) struct Inputs {
    pub(super) by_value_form: &'static str,
    pub(super) conversion: Conversion,
    pub(super) receiver_is_a_temporary: bool,
    pub(super) trait_is_imported: bool,
    pub(super) names_generic_arguments: bool,
}

/// What the rule has to offer for one flagged call.
pub(super) enum Rewrite {
    /// Every edit the change needs, for the fixer to apply.
    Apply(Vec<(Span, String)>),
    /// Nothing to apply, and the rename must not be rendered either:
    /// on its own it changes what the code does. The message says what
    /// would change, so the diagnostic can pass it on.
    Withhold(String),
    /// Nothing to apply, for a reason that leaves the rename worth
    /// showing. Whether to show it is the diagnostic's own call.
    Defer,
}

/// The edits for the whole chain `call` heads, where every part of the
/// change is known.
pub(super) fn rewrite<'tcx>(
    cx: &LateContext<'tcx>,
    call: &'tcx Expr<'tcx>,
    inputs: &Inputs,
) -> Rewrite {
    // The counterpart has to exist where the call is written, and
    // moving the receiver takes nothing away from anyone only where
    // nobody else holds it.
    if !inputs.trait_is_imported || !inputs.receiver_is_a_temporary {
        return Rewrite::Defer;
    }
    let mut parts = match link(
        cx,
        call,
        inputs.by_value_form,
        inputs.conversion,
        inputs.names_generic_arguments,
    ) {
        Rewrite::Apply(edits) => edits,
        declined => return declined,
    };
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
            // command by autoref, as a hand-written call would -- but
            // only where nothing of that name is found by value first.
            let name = method.ident.name;
            if finds_a_by_value_trait_method(cx, parent, name) {
                return Rewrite::Withhold(format!(
                    // `a by-value` rather than `a`/`an` before the
                    // name, which would need the article to agree with
                    // a method name the rule does not choose.
                    "`{name}` ends the chain, and a by-value `{name}` is in scope, so the \
                     owned command this change produces would resolve it differently from \
                     the borrow it replaces",
                ));
            }
            return Rewrite::Apply(parts);
        };
        // A later link resolving anywhere but to `Command`'s own setter
        // is already trait-mediated, and renaming it would move it.
        if !setter::resolves_to_an_inherent_command_method(cx, parent) {
            return Rewrite::Defer;
        }
        match link(
            cx,
            parent,
            by_value_form,
            setter::argument_conversion(cx, parent, arguments),
            method.args.is_some_and(|written| !written.args.is_empty()),
        ) {
            Rewrite::Apply(edits) => parts.extend(edits),
            declined => return declined,
        }
        tail = parent;
    }
    // Nothing further calls it, so the chain's own value is what has to
    // keep its type. The prefix goes in front of the *tail*, not the
    // flagged head: a chain's span usually starts at its head, but a
    // macro invocation wrapping the head alone extends the tail's span
    // to the left of it, and `&mut ` inside the invocation borrows only
    // the part the macro was handed.
    match position(cx, tail) {
        Some(Position::TypeIsKept) => parts.push((tail.span.shrink_to_lo(), "&mut ".to_owned())),
        Some(Position::Discarded) => {}
        None => return Rewrite::Defer,
    }
    Rewrite::Apply(parts)
}

/// The edits one link of the chain needs.
fn link<'tcx>(
    cx: &LateContext<'tcx>,
    call: &'tcx Expr<'tcx>,
    by_value_form: &'static str,
    conversion: Conversion,
    names_generic_arguments: bool,
) -> Rewrite {
    let ExprKind::MethodCall(method, _, arguments, _) = call.kind else {
        return Rewrite::Defer;
    };
    // A written turbofish names the std setter's generic parameters,
    // and the counterpart's do not correspond to them one for one --
    // only `with_envs` takes the same set. Transferring them would be a
    // guess, and dropping them leans on inference, so neither is a
    // rewrite this can promise compiles.
    if names_generic_arguments {
        return Rewrite::Defer;
    }
    // The owned command is created after the arguments are evaluated,
    // where the borrow it replaces was created before them, so it
    // becomes the statement's last temporary and drops first. Only an
    // argument's own temporary is positioned to observe that.
    if arguments
        .iter()
        .any(|argument| creates_an_ordered_drop(cx, argument))
    {
        return Rewrite::Withhold(
            "an argument builds a value whose destructor would run at a different point: \
             the owned command is created after the arguments, where the borrow it \
             replaces was created before them"
                .to_owned(),
        );
    }
    if call.span.from_expansion() || method.ident.span.from_expansion() {
        return Rewrite::Defer;
    }
    let mut edits = vec![(method.ident.span, by_value_form.to_owned())];
    if conversion == Conversion::IntoNeeded {
        let Some(argument) = arguments.first() else {
            return Rewrite::Defer;
        };
        // An argument a macro produced is written in the macro's body,
        // so the conversion would be appended there: to every other
        // expansion of it at once, and to a file the fixer may not
        // even be rewriting. The call's own span says nothing about
        // its arguments', so this is asked separately.
        if argument.span.from_expansion() {
            return Rewrite::Defer;
        }
        edits.push((
            argument.span,
            format!("{}.into()", Sugg::hir(cx, argument, "..").maybe_paren()),
        ));
    }
    Rewrite::Apply(edits)
}

/// Whether the trait methods in scope at `call` include one of this
/// name taking `self` by value.
///
/// The chain's trailing call keeps its name while its receiver becomes
/// an owned `Command`, which moves the method probe's first step from
/// `&mut Command` to `Command`. `Command`'s own methods take `&mut self`
/// or `&self`, so the autoref step still reaches them -- unless some
/// candidate matches by value first, which beats an autoref one at the
/// same step. Only a trait can supply such a candidate, since nobody
/// outside the standard library can write an inherent impl for
/// `Command`. Two of them would be `E0034` and stop the fixer with an
/// error; exactly one compiles and silently calls something else.
///
/// `in_scope_traits` is the set the method probe itself consults, so no
/// trait is weighed that resolution would not weigh. Whether `Command`
/// actually implements the trait is deliberately not asked: answering
/// it means naming the trait's other generic arguments, and where
/// `Command` does not pin them -- `Into` and `TryInto` are the everyday
/// cases, and two impls for `Command` are another -- the answer comes
/// back "no" for want of an inference, which is the wrong way to be
/// wrong. A trait whose method could never apply here costs a declined
/// rewrite instead.
fn finds_a_by_value_trait_method(cx: &LateContext<'_>, call: &Expr<'_>, name: Symbol) -> bool {
    cx.tcx
        .in_scope_traits(call.hir_id)
        .unwrap_or_default()
        .iter()
        .any(|candidate| {
            cx.tcx
                .associated_items(candidate.def_id)
                .filter_by_name_unhygienic(name)
                .any(|item| item.tag() == AssocTag::Fn && takes_self_by_value(cx, item.def_id))
        })
}

/// Whether the first parameter of `method` is `Self` itself rather than
/// a reference to it. Read from the signature, so an `Arc<Self>` or a
/// `Pin<&mut Self>` receiver answers the same as the reference does.
fn takes_self_by_value(cx: &LateContext<'_>, method: DefId) -> bool {
    cx.tcx
        .fn_sig(method)
        .instantiate_identity()
        .skip_binder()
        .inputs()
        .first()
        // `Self` is a trait's own first generic parameter.
        .is_some_and(|first| first.is_param(0))
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
