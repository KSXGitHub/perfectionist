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
const ELISION_NOTE: &str = "a dash between two clauses leaves their relationship for the reader \
                            to work out, and marks a sentence carrying more than one idea";

/// What to do about it. Replaced wholesale by the `help`
/// configuration field.
const RESTRUCTURE_HELP: &str = "one idea per sentence: split any sentence a reader would have to read twice. A comma, \
     colon or semicolon in the dash's place leaves both ideas in the one sentence, so the reader \
     is still left to untangle them";

pub(super) fn emit_dash(
    lint_context: &LateContext<'_>,
    hir_id: HirId,
    span: Span,
    finding: Finding,
    help: Option<&str>,
) {
    let Finding { dash, surface } = finding;
    let help = help.unwrap_or(RESTRUCTURE_HELP).to_owned();
    span_lint_hir_and_then(
        lint_context,
        EM_DASH_PROSE,
        hir_id,
        span,
        format!("{} in {surface}", dash.description()),
        |diag| {
            diag.note(ELISION_NOTE);
            diag.help(help);
        },
    );
}
