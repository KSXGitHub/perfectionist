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
//! The chain's own value still has to land somewhere, which
//! [`position::Position`] answers.
//!
//! The trailing call is the one name the rewrite cannot change, so the
//! chain is declined wherever the traits in scope supply a candidate of
//! that name. [`finds_a_trait_method`] derives why.
//!
//! The names the rewrite does introduce are safe for a different
//! reason. `CommandExtra` has to be imported before a rewrite is built
//! at all, so a competing `with_*` in scope for `Command` leaves two
//! applicable candidates and the renamed call is `E0034` -- an error
//! the fixer reverts, rather than something that quietly resolves
//! elsewhere.

use super::probe::finds_a_trait_method;
use super::setter::{self, Conversion};
use clippy_utils::sugg::Sugg;
use ordering::{creates_an_ordered_drop, leaves_a_sibling_behind};
use position::{Position, position};
use rustc_hir::{Expr, ExprKind, Node};
use rustc_lint::LateContext;
use rustc_span::Span;

mod ordering;
mod position;

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
    // Whether the counterpart is writable here is asked last, by the
    // caller: an import does not make a reordering safe, so a hazard
    // found below has to survive it.
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
    // rewritten takes the whole chain with it.
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
    // declines: a reader renaming by hand reorders the destructors
    // whether or not generic arguments are written, so that line has
    // to be reachable.
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
