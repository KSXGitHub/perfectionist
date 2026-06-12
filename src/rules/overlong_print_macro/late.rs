//! Late-pass machinery: drains the pre-expansion pass's
//! [`PENDING_VIOLATIONS`] queue and emits each diagnostic at the
//! deepest enclosing HIR node, so `cfg_attr`-wrapped `#[expect]` /
//! `#[allow]` attributes resolve correctly. The walk is provided by
//! [`crate::enclosing_hir::find_enclosing_hir_ids`].

use super::PENDING_VIOLATIONS;
use super::emit::emit_fold;
use super::queue::PendingViolation;
use crate::enclosing_hir::find_enclosing_hir_ids;
use rustc_lint::{LateContext, LateLintPass};

pub(super) struct OverlongPrintMacroLate;

impl<'tcx> LateLintPass<'tcx> for OverlongPrintMacroLate {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let pending: Vec<PendingViolation> = {
            let mut guard = PENDING_VIOLATIONS
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            core::mem::take(&mut *guard)
        };
        if pending.is_empty() {
            return;
        }
        let target_spans: Vec<_> = pending
            .iter()
            .map(|violation| violation.call_span)
            .collect();
        let best = find_enclosing_hir_ids(lint_context.tcx, &target_spans);
        for (violation, &hir_id) in pending.into_iter().zip(best.iter()) {
            emit_fold(
                lint_context,
                hir_id,
                violation.call_span,
                violation.replacement,
            );
        }
    }
}
