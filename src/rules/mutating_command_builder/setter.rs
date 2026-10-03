//! Recognising a call this rule is about: which `Command` setter it
//! names, whether it is `Command`'s own setter rather than one an
//! extension trait contributed, and what the by-value counterpart
//! would take where the two signatures differ.
//!
//! The checks are separate because the driver runs them cheapest
//! first: a name, then a type, then a resolution.

use clippy_utils::res::MaybeDef;
use clippy_utils::ty::implements_trait;
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, Mutability};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::{Symbol, sym};

/// `std::process::Command`'s `rustc_diagnostic_item` name. Not among
/// the pre-interned `rustc_span::sym` constants, so it is interned on
/// use, as `needless_borrowed_parameters` does for the same reason.
const COMMAND_DIAGNOSTIC_ITEM: &str = "Command";

/// The `command_extra::CommandExtra` counterpart of a
/// `std::process::Command` setter, or `None` for any other method --
/// `Command::new`, which is not a setter, and the spawning methods,
/// which have no counterpart and take `&mut self` legitimately.
pub(super) fn by_value_form(std_form: Symbol) -> Option<&'static str> {
    Some(match std_form.as_str() {
        "arg" => "with_arg",
        "args" => "with_args",
        "env" => "with_env",
        "envs" => "with_envs",
        "env_remove" => "without_env",
        "env_clear" => "with_no_env",
        "current_dir" => "with_current_dir",
        "stdin" => "with_stdin",
        "stdout" => "with_stdout",
        "stderr" => "with_stderr",
        _ => return None,
    })
}

/// Whether the setter's receiver is an owned command the counterpart
/// could take by value.
///
/// The type is read unadjusted, so an autoref inserted for the
/// `&mut self` signature does not hide an owned receiver -- and so a
/// receiver that is genuinely a `&mut Command` keeps its reference and
/// fails the check, the trait being implemented for the command rather
/// than for a reference to one.
pub(super) fn is_on_an_owned_command<'tcx>(
    cx: &LateContext<'tcx>,
    receiver: &Expr<'_>,
    command_extra_traits: &[DefId],
) -> bool {
    carries_the_counterparts(
        cx,
        cx.typeck_results().expr_ty(receiver),
        command_extra_traits,
    )
}

/// Whether `ty` is a command whose setters this rule speaks about:
/// `std::process::Command` itself, or any type a loaded `CommandExtra`
/// is implemented for.
///
/// Which types those are is the resolved release's decision rather than
/// this rule's. 1.4.0 added `Box<Command>` and, behind a feature,
/// `tokio::process`'s own `Command` and its box; 1.5.0 added
/// `async_process`'s pair. Asking the trait follows all of them without
/// naming a crate here, so neither of those crates has to be reachable
/// for the rule to build, and a release that implements the trait for
/// something new is followed with no change.
///
/// std's `Command` is named by its diagnostic item instead of being
/// asked, because the rule speaks about it whether or not the crate has
/// loaded `command-extra` yet. A crate that has declared the dependency
/// without naming it loads no trait, as
/// [`super::availability::loaded_traits`] explains, and the advice to
/// adopt the by-value form is the whole point of the rule there.
fn carries_the_counterparts<'tcx>(
    cx: &LateContext<'tcx>,
    ty: Ty<'tcx>,
    command_extra_traits: &[DefId],
) -> bool {
    ty.is_diag_item(cx, Symbol::intern(COMMAND_DIAGNOSTIC_ITEM))
        || command_extra_traits
            .iter()
            .any(|command_extra| implements_trait(cx, ty, *command_extra, &[]))
}

/// Whether the call resolves to one of `Command`'s own inherent
/// methods, rather than to a trait method that shares a setter's name.
///
/// The name and the receiver type do not identify the callee. A trait
/// method taking `self` is found at the by-value step of the autoderef
/// chain, *before* `Command`'s own `&mut self` setter, so an extension
/// trait of the author's own with a colliding name wins resolution and
/// must not be renamed.
pub(super) fn resolves_to_an_inherent_command_method<'tcx>(
    cx: &LateContext<'tcx>,
    call: &Expr<'_>,
    command_extra_traits: &[DefId],
) -> bool {
    let Some(method) = cx.typeck_results().type_dependent_def_id(call.hir_id) else {
        return false;
    };
    // Rejected here rather than left to the self-type check below. That
    // check does answer the same today -- resolution records the
    // trait's own declaration, whose `impl_of_assoc` is `None` -- but
    // only because of which `DefId` resolution happens to record: a
    // trait impl's self type is `Command` just as the inherent impl's
    // is, so an impl-item `DefId` would pass it.
    if cx.tcx.trait_of_assoc(method).is_some() {
        return false;
    }
    // What the diagnostic says about the call is that `&mut self` is why
    // the chain cannot yield the command, so a setter of the name that
    // already takes its receiver by value is not this rule's to speak
    // about. Every std setter in the table takes `&mut self`, so this
    // bears only on a command of another crate's, or of the author's.
    if !takes_the_receiver_by_mutable_reference(cx, method) {
        return false;
    }
    cx.tcx.impl_of_assoc(method).is_some_and(|impl_did| {
        carries_the_counterparts(
            cx,
            cx.tcx.type_of(impl_did).skip_binder(),
            command_extra_traits,
        )
    })
}

/// Whether `method`'s own receiver is `&mut self`.
fn takes_the_receiver_by_mutable_reference(cx: &LateContext<'_>, method: DefId) -> bool {
    matches!(
        cx.tcx
            .fn_sig(method)
            .skip_binder()
            .inputs()
            .skip_binder()
            .first()
            .map(|receiver| receiver.kind()),
        Some(ty::Ref(_, _, Mutability::Mut)),
    )
}

/// Whether the by-value counterpart would take this call's argument as
/// it stands.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Conversion {
    /// The argument's type is already the one the counterpart takes.
    None,
    /// The setter is generic over a conversion the counterpart does not
    /// perform, so the argument needs `.into()`.
    IntoNeeded,
}

/// Whether the argument needs converting for the by-value counterpart.
///
/// `Command::stdin`, `stdout` and `stderr` are generic over
/// `Into<Stdio>` where `CommandExtra` takes a concrete `Stdio`. The
/// target type comes from the setter's own bound rather than from a
/// name: `Stdio` carries no `rustc_diagnostic_item`, and reading the
/// predicate cannot fall out of step with the signature the way a
/// spelled-out path could.
///
/// Suggesting `.into()` costs no inference. `stdout<T: Into<Stdio>>`
/// infers `T` from the argument, so an argument that itself needs a
/// target type is circular and ambiguous; the counterpart fixes the
/// target at `Stdio`, so the conversion always has somewhere to land.
/// Every argument the std setter accepts, the converted call accepts
/// too -- and `stdout(f.into())`, which is `E0283`, becomes valid as
/// `with_stdout(f.into())`.
pub(super) fn argument_conversion(
    cx: &LateContext<'_>,
    call: &Expr<'_>,
    arguments: &[Expr<'_>],
) -> Conversion {
    let Some(method) = cx.typeck_results().type_dependent_def_id(call.hir_id) else {
        return Conversion::None;
    };
    let Some(argument) = arguments.first() else {
        return Conversion::None;
    };
    let argument_ty = cx.typeck_results().expr_ty(argument);
    let into_target = cx
        .tcx
        .predicates_of(method)
        .predicates
        .iter()
        .filter_map(|(clause, _)| clause.as_trait_clause())
        .filter(|clause| cx.tcx.is_diagnostic_item(sym::Into, clause.def_id()))
        .find_map(|clause| clause.skip_binder().trait_ref.args.types().nth(1));
    // A setter whose argument already is the target needs nothing; one
    // reaching it through the bound needs `.into()`.
    match into_target {
        Some(target) if argument_ty != target => Conversion::IntoNeeded,
        _ => Conversion::None,
    }
}
