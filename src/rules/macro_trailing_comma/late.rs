//! Late-pass machinery: drains the pre-expansion pass's
//! [`PENDING_VIOLATIONS`] queue and emits
//! each diagnostic at the deepest enclosing HIR node so
//! `cfg_attr`-wrapped `#[expect]` / `#[allow]` attributes resolve
//! correctly. The walk itself is provided by
//! [`crate::enclosing_hir::find_enclosing_hir_ids`].

use rustc_lint::{LateContext, LateLintPass};

use crate::enclosing_hir::find_enclosing_hir_ids;

use super::PENDING_VIOLATIONS;
use super::emit::{emit_insert, emit_remove};
use super::queue::PendingViolation;

pub(super) struct MacroTrailingCommaLate;

impl<'tcx> LateLintPass<'tcx> for MacroTrailingCommaLate {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let pending: Vec<PendingViolation> = {
            let mut guard = PENDING_VIOLATIONS
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            std::mem::take(&mut *guard)
        };
        if pending.is_empty() {
            return;
        }
        let target_spans: Vec<_> = pending.iter().map(|violation| violation.span()).collect();
        let best = find_enclosing_hir_ids(lint_context.tcx, &target_spans);
        for (violation, &hir_id) in pending.iter().zip(best.iter()) {
            match *violation {
                PendingViolation::Insert(span) => emit_insert(lint_context, hir_id, span),
                PendingViolation::Remove(span) => emit_remove(lint_context, hir_id, span),
            }
        }
    }
}
