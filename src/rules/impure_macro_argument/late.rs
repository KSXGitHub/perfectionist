//! Late-pass machinery: drains the pre-expansion pass's
//! [`PENDING_VIOLATIONS`] queue and emits
//! each diagnostic at the deepest enclosing HIR node so
//! `cfg_attr`-wrapped `#[expect]` / `#[allow]` attributes resolve
//! correctly. The walk itself is provided by
//! [`crate::enclosing_hir::find_enclosing_hir_ids`].

use super::{IMPURE_MACRO_ARGUMENT, PENDING_VIOLATIONS};
use crate::enclosing_hir::find_enclosing_hir_ids;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

pub(super) struct ImpureMacroArgumentLate;

impl<'tcx> LateLintPass<'tcx> for ImpureMacroArgumentLate {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let pending: Vec<Span> = {
            let mut guard = PENDING_VIOLATIONS
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            core::mem::take(&mut *guard)
        };
        if pending.is_empty() {
            return;
        }
        let best = find_enclosing_hir_ids(lint_context.tcx, &pending);
        for (&span, &hir_id) in pending.iter().zip(best.iter()) {
            emit(lint_context, hir_id, span);
        }
    }
}

fn emit(lint_context: &LateContext<'_>, hir_id: hir::HirId, span: Span) {
    span_lint_hir_and_then(
        lint_context,
        IMPURE_MACRO_ARGUMENT,
        hir_id,
        span,
        "impure expression passed directly to a macro",
        |diag| {
            diag.help(
                "bind the expression to a `let` immediately before the macro \
                 call so it is evaluated exactly once, regardless of how the \
                 macro expands",
            );
        },
    );
}
