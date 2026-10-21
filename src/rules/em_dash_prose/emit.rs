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

/// Why a dash in running prose is worth a warning: one line per
/// reason, since no one of them carries the whole case. Stated once
/// each, as they read the same for every dash the rule flags.
const REASON_NOTES: &[&str] = &[
    "a dash between two clauses leaves their relationship for the reader to work out, and marks a \
     sentence carrying more than one idea",
    "a colon, a semicolon, a comma and a pair of parentheses each name a particular relationship, \
     so a dash in the place of one of them says only that some relationship holds",
    "a comment or a doc comment is technical writing rather than editorial prose: it is read for \
     ease of understanding, and the dash is a device of voice",
];

/// What to do about it: the caution first, then the principle, then
/// one line per situation, each stating the condition it holds under.
/// None of them commands a fix. Which one suits a given sentence
/// depends on what the dash is standing in for, which the rule cannot
/// see, so the lines inform the reader's judgement rather than direct
/// it. All of them are replaced wholesale by the `help` configuration
/// field.
const RESTRUCTURE_HELPS: &[&str] = &[
    "a mechanical swap silences this warning and leaves the phrasing in place; which fix suits the \
     sentence is a judgement this warning cannot make",
    "one idea per sentence is what reads easily, so a dash is often a sign the sentence wants \
     dividing rather than repunctuating",
    "where the clause after the dash elaborates the one before it, a colon names that relationship",
    "where both clauses stand on their own, a semicolon joins them or a full stop makes them two \
     sentences",
    "where a pair of dashes fences an aside, commas fold it in or a sentence of its own carries it",
];

pub(super) fn emit_dash(
    lint_context: &LateContext<'_>,
    hir_id: HirId,
    span: Span,
    finding: Finding,
    help: Option<&str>,
) {
    let Finding { dash, surface } = finding;
    let help = help.map(ToOwned::to_owned);
    span_lint_hir_and_then(
        lint_context,
        EM_DASH_PROSE,
        hir_id,
        span,
        format!("{} in {surface}", dash.description()),
        |diag| {
            for note in REASON_NOTES {
                diag.note(*note);
            }
            match &help {
                Some(help) => {
                    diag.help(help.clone());
                }
                None => {
                    for help in RESTRUCTURE_HELPS {
                        diag.help(*help);
                    }
                }
            }
        },
    );
}
