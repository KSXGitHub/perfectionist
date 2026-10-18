//! Diagnostic emission for the one violation shape this rule has: a
//! dash standing between two clauses.
//!
//! The diagnostic carries no suggestion, and that is the rule's whole
//! point rather than an omission. Swapping the dash for a comma or a
//! colon would silence the warning and leave the loose phrasing it
//! reports in place, so there is nothing for `cargo clippy --fix` or
//! any other rewriting tool to apply.

use super::EM_DASH_PROSE;
use super::finding::Finding;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_hir::HirId;
use rustc_lint::LateContext;
use rustc_span::Span;

/// Why a dash in running prose is worth a warning. Stated once, since
/// it reads the same for every dash the rule flags.
const ELISION_NOTE: &str = "a dash between two clauses leaves the relationship between them \
                            unstated, which is the register AI-generated prose settles into; \
                            deliberate technical writing names that relationship";

/// What to do about it. Replaced wholesale by the `message`
/// configuration field.
const RESTRUCTURE_HELP: &str = "reword so each clause stands on its own, rather than replacing the dash in place: split \
     the sentence in two, or pick the mark that names the relationship the dash left unstated (a \
     colon introduces an elaboration, a semicolon joins parallel statements, a comma attaches a \
     dependent clause)";

/// Addressed to the readership this rule was written for. An assistant
/// that substitutes a different punctuation character has read the
/// diagnostic as a spelling correction, which is the one reading it is
/// not.
const ASSISTANT_NOTE: &str = "an assistant meeting this diagnostic in its own output is being \
                              asked to rewrite the surrounding paragraph, not to substitute \
                              another punctuation character";

pub(super) fn emit_dash(
    lint_context: &LateContext<'_>,
    hir_id: HirId,
    span: Span,
    finding: Finding,
    message: Option<&str>,
) {
    let Finding { dash, surface } = finding;
    let help = message.unwrap_or(RESTRUCTURE_HELP).to_owned();
    span_lint_hir_and_then(
        lint_context,
        EM_DASH_PROSE,
        hir_id,
        span,
        format!("{} in {surface}", dash.description()),
        |diag| {
            diag.note(ELISION_NOTE);
            diag.help(help);
            diag.note(ASSISTANT_NOTE);
        },
    );
}
