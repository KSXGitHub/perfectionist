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
//! receiver becomes an owned `Command` where it was a `&mut Command`.
//! That moves what the method probe finds, so the chain is declined
//! wherever the traits in scope supply a candidate of that name.
//! [`finds_a_trait_method`] derives why.
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
use rustc_hir::{Expr, ExprKind, MatchSource, Node, StmtKind, UnOp};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_middle::ty::adjustment::Adjust;
use rustc_span::{Ident, Span, Symbol};

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
    match edits(cx, call, inputs) {
        Rewrite::Apply(parts) if inputs.trait_is_imported => Rewrite::Apply(parts),
        // Nothing to apply until the trait is in scope, but the rename
        // is still worth showing, and the remedy says what is missing.
        Rewrite::Apply(_) => Rewrite::Defer,
        declined => declined,
    }
}

/// The edits themselves, asked without regard to whether the
/// counterpart can be written here yet.
fn edits<'tcx>(cx: &LateContext<'tcx>, call: &'tcx Expr<'tcx>, inputs: &Inputs) -> Rewrite {
    // The counterpart has to exist where the call is written, and
    // moving the receiver takes nothing away from anyone only where
    // nobody else holds it.
    // Whether the counterpart is writable here is asked last, by the
    // caller: an import does not make a reordering safe, so a hazard
    // found below has to survive it. Sending the reader to fetch an
    // import and then handing them a rename the rule would have
    // refused is the worst of both.
    if !inputs.receiver_is_a_temporary {
        return Rewrite::Defer;
    }
    if let ExprKind::MethodCall(_, receiver, ..) = call.kind
        && leaves_a_sibling_behind(cx, receiver)
    {
        return Rewrite::Withhold(
            "the command is a field of a temporary that leaves behind something with a \
             destructor, and the change would drop the command before it rather than as \
             part of the temporary"
                .to_owned(),
        );
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
            // only where nothing else of that name is found first.
            let name = method.ident.name;
            if finds_a_trait_method(cx, parent, name) {
                return Rewrite::Withhold(format!(
                    // No article before the name, which would have to
                    // agree with a method the rule does not choose.
                    "`{name}` ends the chain, and `{name}` is declared by a trait in scope, \
                     so the owned command this change produces may resolve it differently \
                     from the borrow it replaces",
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
    // The owned command is created after the arguments are evaluated,
    // where the borrow it replaces was created before them, so it
    // becomes the statement's last temporary and drops first. Only a
    // value the call leaves behind is positioned to observe that.
    //
    // Asked before the generic-arguments check below, which also
    // declines: a reader
    // renaming by hand reorders the destructors whether or not generic
    // arguments are written, so that line has to be reachable.
    if arguments
        .iter()
        .any(|argument| creates_an_ordered_drop(cx, argument))
    {
        let name = method.ident.name;
        return Rewrite::Withhold(format!(
            "`{name}`'s argument leaves behind a value with a destructor, and the change \
             would drop it after the command rather than before it",
        ));
    }
    // Written generic arguments name the std setter's parameters, and
    // the counterpart's do not correspond to them one for one --
    // only `with_envs` takes the same set. Transferring them would be a
    // guess, and dropping them leans on inference, so neither is a
    // rewrite this can promise compiles.
    if names_generic_arguments {
        return Rewrite::Defer;
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
/// name.
///
/// The chain's trailing call keeps its name while its receiver becomes
/// an owned `Command`, which moves the method probe's first step from
/// `&mut Command` to `Command`. Against a `&mut Command`,
/// `Command::status(&mut self)` matches at step 0 *by value*, because
/// the step type already is the `&mut Command` its receiver wants --
/// the first pick the probe tries, which nothing can get ahead of.
/// Against a `Command` it matches nothing until the `&mut` autoref, the
/// *last* of the three picks at that step, so a candidate taking `self`
/// or `&self` is found first. Only a trait can supply such a candidate,
/// since nobody outside the standard library can write an inherent impl
/// for `Command`. Two of them would be `E0034` and stop the fixer with
/// an error; exactly one compiles and silently calls something else.
///
/// `in_scope_traits` is the set the method probe itself consults, so
/// no trait is weighed that resolution would not weigh. Two things it
/// deliberately does not ask, both because the safe answer is the one
/// that declines:
///
/// - Whether `Command` implements the trait. Answering means naming
///   the trait's other generic arguments, and where `Command` does not
///   pin them -- `Into` and `TryInto` are the everyday cases -- the
///   answer comes back "no" for want of an inference.
/// - What the method's receiver is. `self` and `&self` both reach the
///   owned command ahead of the inherent setter, and `&mut self` ties
///   with it and loses. Declining on the receiver shape alone would be
///   unsound, though, because an inherent `&self` method loses to a
///   trait `&mut self` one before the change and wins after. The sound
///   refinement is narrower -- no inherent method of the name, and
///   every candidate taking `&mut self` -- and what it would buy is
///   `CommandExt::exec` and its neighbours, which this costs today.
///
/// `is_method` is what keeps an associated function out, since its
/// `has_self` is exactly the set `value.name()` can reach: a
/// `fn status(this: Self)` is excluded and an arbitrary self type is
/// not.
pub(super) fn finds_a_trait_method(cx: &LateContext<'_>, call: &Expr<'_>, name: Symbol) -> bool {
    cx.tcx
        .in_scope_traits(call.hir_id)
        .unwrap_or_default()
        .iter()
        .any(|candidate| {
            cx.tcx
                .associated_items(candidate.def_id)
                .filter_by_name_unhygienic(name)
                .any(rustc_middle::ty::AssocItem::is_method)
        })
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
///
/// Only ever asked about the chain's tail, and the walk that finds the
/// tail has already followed every method call taking the value as its
/// receiver -- so no parent here is one of those.
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
        // carry parentheses -- and `(&mut command).field` reads worse
        // than the expression it replaces, which is the whole point of
        // the rule. The diagnostic shows the bare rename there instead.
        Node::Expr(Expr {
            kind: ExprKind::Field(base, _) | ExprKind::Index(base, ..),
            ..
        }) if base.hir_id == call.hir_id => None,
        Node::Expr(_) => Some(Position::TypeIsKept),
        _ => None,
    }
}

/// Whether evaluating `argument` leaves behind a value whose destructor
/// runs at a point the rewrite would move.
fn creates_an_ordered_drop<'tcx>(cx: &LateContext<'tcx>, argument: &'tcx Expr<'tcx>) -> bool {
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
fn leaves_a_sibling_behind<'tcx>(cx: &LateContext<'tcx>, receiver: &'tcx Expr<'tcx>) -> bool {
    let mut carried = receiver;
    while let ExprKind::Field(base, field) = carried.kind {
        if leaves_a_droppable_sibling(cx, base, field) {
            return true;
        }
        carried = base;
    }
    false
}
