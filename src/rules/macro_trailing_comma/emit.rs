//! Diagnostic emission for the two violation shapes.
//!
//! Both suggestions are `MachineApplicable`, and cannot ping-pong with
//! rustfmt: rustfmt strips a trailing comma only where it rewrites a
//! `vec!` as an array literal, and its vertical policy there already
//! matches this lint's. Everywhere else rustfmt keeps the comma it
//! finds, so nothing it does undoes either fix. Measured over a sweep
//! of widths across rustfmt's line-breaking threshold, repeated under
//! each layout knob: no fixture ever returned to an earlier state.

use super::MACRO_TRAILING_COMMA;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::LateContext;
use rustc_span::Span;

pub(super) fn emit_insert(lint_context: &LateContext<'_>, hir_id: hir::HirId, insert_at: Span) {
    span_lint_hir_and_then(
        lint_context,
        MACRO_TRAILING_COMMA,
        hir_id,
        insert_at,
        "multi-line macro invocation should end with a trailing comma",
        |diag| {
            diag.span_suggestion(
                insert_at,
                "add a trailing comma",
                ",",
                Applicability::MachineApplicable,
            );
        },
    );
}

pub(super) fn emit_remove(lint_context: &LateContext<'_>, hir_id: hir::HirId, comma_span: Span) {
    span_lint_hir_and_then(
        lint_context,
        MACRO_TRAILING_COMMA,
        hir_id,
        comma_span,
        "single-line macro invocation should not end with a trailing comma",
        |diag| {
            diag.span_suggestion(
                comma_span,
                "remove the trailing comma",
                "",
                Applicability::MachineApplicable,
            );
        },
    );
}
