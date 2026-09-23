//! Turning one flagged setter call into its diagnostic.
//!
//! Everything the diagnostic reads about the call is decided before it
//! gets here, so this module holds no analysis. What it picks is how
//! much of the change to show, and it names the part the rename does
//! not cover wherever it knows which part that is: "this is not just a
//! rename" is not something a reader can act on.

use super::MUTATING_COMMAND_BUILDER;
use super::fix::Rewrite;
use super::setter::Conversion;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::{Applicability, Diag};
use rustc_hir::HirId;
use rustc_lint::LateContext;
use rustc_span::{Span, Symbol};

/// Said wherever written generic arguments outlive the rename.
const GENERIC_ARGUMENTS: &str = "the call names its generic arguments, which are this setter's, \
                                 so check them against the counterpart's when you rename";

/// Where a flagged call's value lands, in the terms the diagnostic
/// needs. The distinction `MovesALaterCall` draws is that a rendered
/// rename edits one method segment: where a later call takes this
/// one's value, that call is then written against a receiver the
/// rename has just made owned, and a by-value method of its name in
/// scope is found there ahead of `Command`'s own.
#[derive(PartialEq, Eq, Clone, Copy)]
pub(super) enum Landing {
    /// A position that takes the owned command, where renaming this
    /// call alone leaves every other call calling what it called.
    TakesTheRename,
    /// A position that takes the owned command, but a later call in
    /// the chain would resolve somewhere new. Carries that call's
    /// name, which is the part a reader has to check.
    MovesALaterCall(Symbol),
    /// Anywhere else -- a `let`, a call argument, a struct field --
    /// or a position a macro's own body supplied.
    Unknown,
}

/// One flagged setter call, with every question about it already
/// answered.
pub(super) struct Violation {
    /// The node the diagnostic hangs off, so that an `#[allow(...)]`
    /// written around the call still applies to it.
    pub(super) hir_id: HirId,
    /// The method segment alone, which is the span a rename replaces.
    pub(super) method_span: Span,
    /// The setter as written.
    pub(super) std_form: Symbol,
    /// The `CommandExtra` method to name in its place.
    pub(super) by_value_form: &'static str,
    /// Whether the argument needs `.into()`.
    pub(super) conversion: Conversion,
    /// Whether the call names its generic arguments.
    pub(super) names_generic_arguments: bool,
    /// Whether the receiver is a temporary.
    pub(super) receiver_is_a_temporary: bool,
    /// Where the call's value lands.
    pub(super) landing: Landing,
    /// Which remedy to name, where the counterpart is not yet writable
    /// here, and `None` where it is.
    pub(super) remedy: Option<&'static str>,
    /// What the rule has to offer.
    pub(super) rewrite: Rewrite,
}

/// The lines come out in the order a reader works through them: which
/// method to use, then what the rename does not cover, then how to
/// reach the method at all. A rendered rename carries its own span, and
/// rustc prints every span-less line above such a block, so where the
/// rename is rendered the remedy has to travel inside its message to
/// stay in that order.
pub(super) fn violation(cx: &LateContext<'_>, violation: Violation) {
    let Violation {
        hir_id,
        method_span,
        std_form,
        by_value_form,
        conversion,
        names_generic_arguments,
        receiver_is_a_temporary,
        landing,
        remedy,
        rewrite,
    } = violation;
    let violation_lines = Lines {
        names_generic_arguments,
        receiver_is_a_temporary,
        landing,
        remedy,
    };
    // Rendering the rename is not a claim that it is the whole change:
    // an import may be needed alongside it, which `remedy` names.
    let show_the_rename = conversion == Conversion::None
        && !names_generic_arguments
        && receiver_is_a_temporary
        && landing == Landing::TakesTheRename;
    span_lint_hir_and_then(
        cx,
        MUTATING_COMMAND_BUILDER,
        hir_id,
        method_span,
        format!("`Command::{std_form}` takes `&mut self`, so it cannot yield the command"),
        |diagnostic| {
            let advice = match conversion {
                Conversion::None => format!(
                    "use `CommandExtra::{by_value_form}`, which takes `self` and returns \
                     `Self`, keeping the whole construction in expression position",
                ),
                Conversion::IntoNeeded => format!(
                    "use `CommandExtra::{by_value_form}`, which takes `self` and returns \
                     `Self`; it takes the concrete type this setter converts into, so \
                     convert the argument with `.into()`",
                ),
            };
            match rewrite {
                // Every part known, so hand the fixer the whole edit
                // rather than describing what it would take.
                Rewrite::Apply(edits) => {
                    diagnostic.multipart_suggestion(
                        advice,
                        edits,
                        Applicability::MachineApplicable,
                    );
                    return;
                }
                // The rename is the part that would change what the
                // code does, so rendering it would put the hazard back
                // in front of the reader as a line to copy. The advice
                // stands; what stopped it is said instead.
                Rewrite::Withhold {
                    reason,
                    is_the_landing,
                } => {
                    diagnostic.help(advice);
                    if names_generic_arguments {
                        diagnostic.help(GENERIC_ARGUMENTS);
                    }
                    // The advice still says to rename, so the reader
                    // walks into the re-resolution the rendered form
                    // is withheld for. Withholding the edit is not
                    // withholding the warning -- except where the
                    // reason is that warning already.
                    if !is_the_landing && let Landing::MovesALaterCall(next) = landing {
                        diagnostic.help(moves_a_later_call(next));
                    }
                    diagnostic.help(reason);
                    if let Some(remedy) = remedy {
                        diagnostic.help(remedy);
                    }
                    return;
                }
                Rewrite::Defer => {}
            }
            match show_the_rename {
                true => {
                    let advice = match remedy {
                        Some(remedy) => format!("{advice}; {remedy}"),
                        None => advice,
                    };
                    diagnostic.span_suggestion(
                        method_span,
                        advice,
                        by_value_form,
                        Applicability::MaybeIncorrect,
                    );
                }
                // The argument needing `.into()` is the line the
                // advice above already carries.
                false => prose(diagnostic, &violation_lines, advice),
            }
        },
    );
}

/// The hazard a hand-written rename walks into where the chain's next
/// call is one a trait in scope also declares. Both branches that
/// leave the reader to rename say it, so it is written once.
fn moves_a_later_call(next: Symbol) -> String {
    format!(
        "`{next}` takes this call's value, and `{next}` is declared by a \
         trait in scope, so renaming this call alone may make that one \
         resolve differently",
    )
}

/// What the prose branch reads.
struct Lines {
    names_generic_arguments: bool,
    receiver_is_a_temporary: bool,
    landing: Landing,
    remedy: Option<&'static str>,
}

/// The lines for a change the rule can describe but not render: each
/// thing the change reaches past earns one.
fn prose(diagnostic: &mut Diag<'_, ()>, lines: &Lines, advice: String) {
    let &Lines {
        names_generic_arguments,
        receiver_is_a_temporary,
        landing,
        remedy,
    } = lines;
    diagnostic.help(advice);
    if names_generic_arguments {
        diagnostic.help(GENERIC_ARGUMENTS);
    }
    if let Landing::MovesALaterCall(next) = landing {
        diagnostic.help(moves_a_later_call(next));
    }
    if !receiver_is_a_temporary {
        diagnostic.help(
            "the receiver outlives this call, so where later code reads it, \
             the change also has to reassign it or collapse the statements \
             into one chained expression",
        );
    }
    if landing == Landing::Unknown {
        // One line for both reasons a position does not take the
        // value -- it is not such a position, or a macro wrote it --
        // since two lines read as competing answers where a macro uses
        // one written expression twice, both uses carrying the same
        // span. It claims neither that the change reaches further nor
        // that the rename settles it: over a discarded value the
        // rename is the whole change, and beside the lines above it is
        // not.
        diagnostic.help(
            "whether the change reaches further depends on what the \
             surrounding code does with this call's value",
        );
    }
    if let Some(remedy) = remedy {
        diagnostic.help(remedy);
    }
}
