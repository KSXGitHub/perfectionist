//! Diagnostic emission for the two violation shapes.

use super::MACRO_TRAILING_COMMA;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::LateContext;
use rustc_span::Span;

/// Report a multi-line invocation that does not end with a trailing
/// comma, suggesting one at `insert_at`.
///
/// **`MachineApplicable` autofix is sound:**
///
/// The suggestion cannot start an exchange with rustfmt. Where rustfmt
/// has settled the layout, breaking this invocation across lines was
/// its own choice, taken from argument widths that exclude the trailing
/// comma, so adding one cannot pull the invocation back onto a single
/// line. It stays multi-line, the layout where rustfmt either keeps the
/// comma it finds or writes one itself, so [`emit_remove`] is left no
/// single-line invocation to undo this from.
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

/// Report a single-line invocation that ends with a trailing comma,
/// suggesting the deletion of `comma_span`.
///
/// **`MachineApplicable` autofix is sound:**
///
/// The suggestion cannot start an exchange with rustfmt. Where rustfmt
/// has settled the layout, keeping this invocation on one line was its
/// own choice, taken from argument widths that exclude the trailing
/// comma, so deleting one cannot push the invocation over that width
/// and break it. It stays single-line, the layout where rustfmt either
/// leaves the comma out or strips one itself, so [`emit_insert`] is
/// left no multi-line invocation to undo this from.
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
