//! Diagnostic emission for the single violation shape this rule has:
//! a long-line print macro whose template embeds a newline.

use super::PRINT_MACRO_SPLIT;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::LateContext;
use rustc_span::Span;

pub(super) fn emit_fold(
    lint_context: &LateContext<'_>,
    hir_id: hir::HirId,
    call_span: Span,
    replacement: String,
) {
    span_lint_hir_and_then(
        lint_context,
        PRINT_MACRO_SPLIT,
        hir_id,
        call_span,
        "print macro with an embedded newline exceeds the configured line width",
        |diag| {
            diag.span_suggestion(
                call_span,
                "fold the template across lines with a backslash-newline continuation",
                replacement,
                Applicability::MachineApplicable,
            );
        },
    );
}
