//! Diagnostic emission for the consumed-literal violations the
//! pre-expansion pass discovers. The late `ExprKind::Lit` path emits its
//! own diagnostics inline via `span_lint_and_sugg`; this helper is the
//! HIR-anchored counterpart for the queued ones, drained at
//! `check_crate_post`. Message, label, and applicability match the
//! inline path exactly so the two firing routes are indistinguishable
//! to the user.

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::LateContext;
use rustc_span::Span;

use super::{PREFER_RAW_STRING, SUGGESTION_LABEL, VIOLATION_MESSAGE};

pub(super) fn emit_raw_string(
    lint_context: &LateContext<'_>,
    hir_id: hir::HirId,
    span: Span,
    suggestion: String,
) {
    span_lint_hir_and_then(
        lint_context,
        PREFER_RAW_STRING,
        hir_id,
        span,
        VIOLATION_MESSAGE,
        |diag| {
            diag.span_suggestion(
                span,
                SUGGESTION_LABEL,
                suggestion,
                Applicability::MachineApplicable,
            );
        },
    );
}
