//! Turning one flagged setter call into its diagnostic.
//!
//! Everything the diagnostic reads about the call is decided before it
//! gets here, so this module holds no analysis: it chooses between
//! rendering the rename and describing the change, and where it
//! describes, says which part of the change the rename does not cover.
//! "This is not just a rename" is not something a reader can act on.

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
    /// The span of the position the call's value lands in, where that
    /// position takes an owned `Command`. `None` where no position
    /// does; a span that is `from_expansion` where a macro wrote the
    /// position rather than the author.
    pub(super) position: Option<Span>,
    /// Which remedy to name, where the counterpart is not yet writable
    /// here, and `None` where it is.
    pub(super) remedy: Option<&'static str>,
}

pub(super) fn violation(cx: &LateContext<'_>, violation: Violation) {
    let Violation {
        hir_id,
        method_span,
        std_form,
        by_value_form,
        conversion,
        names_generic_arguments,
        receiver_is_a_temporary,
        position,
        remedy,
    } = violation;
    let position_is_written_here = position.is_some_and(|span| !span.from_expansion());
    // Rendering the rename is not a claim that it is the whole change:
    // an import may be needed alongside it, which `remedy` names.
    let show_the_rename = conversion == Conversion::None
        && !names_generic_arguments
        && receiver_is_a_temporary
        && position_is_written_here;
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
                            "the receiver outlives this call, so finish the change by \
                             reassigning it or by collapsing the binding into one chained \
                             expression",
                        );
                    }
                    if !position_is_written_here {
                        diagnostic.help(match position {
                            // The position does take the owned command;
                            // a macro wrote it.
                            Some(_) => {
                                "the position taking this call's value is written in a macro, \
                                 so a rename shown here would be written over every use the \
                                 macro makes of the expression"
                            }
                            None => {
                                "what else the change takes depends on what the surrounding \
                                 code does with this call's value"
                            }
                        });
                    }
                }
            }
            if let Some(remedy) = remedy {
                diagnostic.help(remedy);
            }
        },
    );
}
