//! Recognising a call this rule is about: which `Command` setter it
//! names, whether it is `Command`'s own setter rather than one an
//! extension trait contributed, and what the by-value counterpart
//! would take where the two signatures differ.
//!
//! The three checks are separate because the driver runs them cheapest
//! first: a name, then a type, then a resolution.

use clippy_utils::res::MaybeDef;
use rustc_hir::Expr;
use rustc_lint::LateContext;
use rustc_span::Symbol;

/// `std::process::Command`'s `rustc_diagnostic_item` name. Not among
/// the pre-interned `rustc_span::sym` constants, so it is interned on
/// use, as `needless_borrowed_parameters` does for the same reason.
const COMMAND_DIAGNOSTIC_ITEM: &str = "Command";

/// `std::convert::Into`'s `rustc_diagnostic_item` name, interned on
/// use for the same reason.
const INTO_DIAGNOSTIC_ITEM: &str = "Into";

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

/// Whether the setter's receiver is an owned `Command`.
///
/// The type is read unadjusted, so an autoref inserted for the
/// `&mut self` signature does not hide an owned receiver -- and so a
/// receiver that is genuinely a `&mut Command` keeps its reference and
/// fails the check.
pub(super) fn is_on_an_owned_command(cx: &LateContext<'_>, receiver: &Expr<'_>) -> bool {
    cx.typeck_results()
        .expr_ty(receiver)
        .is_diag_item(cx, Symbol::intern(COMMAND_DIAGNOSTIC_ITEM))
}

/// Whether the call resolves to one of `Command`'s own inherent
/// methods, rather than to a trait method that shares a setter's name.
///
/// The name and the receiver type do not identify the callee. A trait
/// method taking `self` is found at the by-value step of the autoderef
/// chain, *before* `Command`'s own `&mut self` setter, so an extension
/// trait of the author's own with a colliding name wins resolution and
/// must not be renamed.
pub(super) fn resolves_to_an_inherent_command_method(
    cx: &LateContext<'_>,
    call: &Expr<'_>,
) -> bool {
    let Some(method) = cx.typeck_results().type_dependent_def_id(call.hir_id) else {
        return false;
    };
    // A trait impl's self type is `Command` too, so the self type alone
    // does not separate the inherent setter from an extension trait's.
    if cx.tcx.trait_of_assoc(method).is_some() {
        return false;
    }
    cx.tcx.impl_of_assoc(method).is_some_and(|impl_did| {
        cx.tcx
            .type_of(impl_did)
            .skip_binder()
            .is_diag_item(cx, Symbol::intern(COMMAND_DIAGNOSTIC_ITEM))
    })
}

/// Whether the by-value counterpart would take this call's argument as
/// it stands.
#[derive(PartialEq, Eq)]
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
        .filter(|clause| {
            cx.tcx
                .is_diagnostic_item(Symbol::intern(INTO_DIAGNOSTIC_ITEM), clause.def_id())
        })
        .find_map(|clause| clause.skip_binder().trait_ref.args.types().nth(1));
    // A setter whose argument already is the target needs nothing; one
    // reaching it through the bound needs `.into()`.
    match into_target {
        Some(target) if argument_ty != target => Conversion::IntoNeeded,
        _ => Conversion::None,
    }
}
