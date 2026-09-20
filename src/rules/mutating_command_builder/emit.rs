//! Turning one flagged setter call into its diagnostic.
//!
//! Everything the diagnostic reads about the call is decided before it
//! gets here, so this module holds no analysis: it chooses between
//! rendering the rename and describing the change, and where it
//! describes, names the part of the change the rename does not cover
//! wherever it knows which part that is. "This is not just a rename"
//! is not something a reader can act on.

use super::MUTATING_COMMAND_BUILDER;
use super::setter::Conversion;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir::HirId;
use rustc_lint::LateContext;
use rustc_span::{Span, Symbol};

/// One flagged setter call, with every question about it already
/// answered.
pub(super) struct Violation {
    /// The node the diagnostic hangs off, so that an `#[allow(...)]`
    /// written around the call still applies to it.
    pub(super) hir_id: HirId,
    /// The method segment alone, which is the span a rename replaces
    /// and narrower than the call it belongs to.
    pub(super) method_span: Span,
    /// The setter as written.
    pub(super) std_form: Symbol,
    /// The `CommandExtra` method to name in its place.
    pub(super) by_value_form: &'static str,
    /// Whether the counterpart would take the argument as it stands.
    pub(super) conversion: Conversion,
    /// Whether the call names its generic arguments. They are the std
    /// setter's, and they survive a rename of the segment alone.
    pub(super) names_generic_arguments: bool,
    /// Whether the receiver is a value the expression produced rather
    /// than a place the surrounding code still holds.
    pub(super) receiver_is_a_temporary: bool,
    /// Whether the call's value lands in a position that takes an owned
    /// `Command` *and* that position is written where the call is.
    pub(super) position_takes_it: bool,
    /// Which remedy to name, where the counterpart is not yet writable
    /// here, and `None` where it is.
    pub(super) remedy: Option<&'static str>,
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
        position_takes_it,
        remedy,
    } = violation;
    // Rendering the rename is not a claim that it is the whole change:
    // an import may be needed alongside it, which `remedy` names.
    let show_the_rename = conversion == Conversion::None
        && !names_generic_arguments
        && receiver_is_a_temporary
        && position_takes_it;
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
                     `Self`; it takes the argument by value, so convert it with `.into()`",
                ),
            };
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
                // Each thing the change reaches past earns a line, and
                // the argument needing `.into()` is the one the advice
                // above already carries.
                false => {
                    diagnostic.help(advice);
                    if names_generic_arguments {
                        diagnostic.help(
                            "the call names its generic arguments, which are this setter's, \
                             so check them against the counterpart's when you rename",
                        );
                    }
                    if !receiver_is_a_temporary {
                        diagnostic.help(
                            "the receiver outlives this call, so the change also has to \
                             reassign it or collapse the binding into one chained \
                             expression",
                        );
                    }
                    if !position_takes_it {
                        // One line for both of the reasons a position
                        // does not take the value -- it is not one of
                        // the two that do, or a macro wrote it -- since
                        // two lines read as two competing answers where
                        // a macro uses one written expression twice,
                        // both uses carrying the same span. It claims
                        // neither that the change reaches further nor
                        // that the rename settles it: over a discarded
                        // value the rename is the whole change, and
                        // beside the lines above it is not.
                        diagnostic.help(
                            "whether the change reaches further depends on what the \
                             surrounding code does with this call's value",
                        );
                    }
                    if let Some(remedy) = remedy {
                        diagnostic.help(remedy);
                    }
                }
            }
        },
    );
}
