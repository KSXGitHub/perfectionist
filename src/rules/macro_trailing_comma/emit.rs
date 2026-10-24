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
/// **[`MachineApplicable`](Applicability::MachineApplicable) does not fight rustfmt:**
///
/// On source rustfmt has already formatted, the layout is rustfmt's
/// own choice. It broke this invocation across lines by measuring the
/// arguments, and that measurement leaves the trailing comma out. So
/// adding a comma cannot change the choice: the invocation stays
/// multi-line.
///
/// Multi-line is the layout rustfmt wants a trailing comma in. It
/// keeps one already there, and writes one itself where it lays the
/// arguments out. The comma therefore survives formatting, and
/// [`emit_remove`] fires only on single-line invocations, so nothing
/// takes it away again.
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
/// **[`MachineApplicable`](Applicability::MachineApplicable) does not fight rustfmt:**
///
/// On source rustfmt has already formatted, the layout is rustfmt's
/// own choice. It kept this invocation on one line by measuring the
/// arguments, and that measurement leaves the trailing comma out. So
/// deleting a comma cannot change the choice: the invocation stays
/// single-line.
///
/// Single-line is the layout rustfmt wants no trailing comma in. It
/// never adds one, and strips one where it lays the arguments out. The
/// comma therefore stays gone, and [`emit_insert`] fires only on
/// multi-line invocations, so nothing puts it back.
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
