//! Diagnostic emission for `unpinned_repo_ref`. One message shape per
//! [`RefProblem`], all sharing the same help line. There is no
//! autofix: resolving a branch or tag to a SHA would require querying
//! the remote (out of scope for a Dylint pass, and the URL may point
//! at a different repository than the one being linted).

use super::UNPINNED_REPO_REF;
use crate::rules::unpinned_repo_ref::parse::RefProblem;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_hir::HirId;
use rustc_lint::LateContext;
use rustc_span::Span;

/// One unpinned-ref finding: what's wrong with the ref and its text.
pub(super) struct Violation {
    pub(super) problem: RefProblem,
    pub(super) reference: String,
}

pub(super) fn emit_diagnostic(
    lint_context: &LateContext<'_>,
    hir_id: HirId,
    span: Span,
    violation: &Violation,
) {
    let Violation { problem, reference } = violation;
    let message = match problem {
        RefProblem::Branch => {
            format!("repository URL references branch `{reference}` instead of a commit SHA")
        }
        RefProblem::Tag => {
            format!("repository URL references tag `{reference}` instead of a commit SHA")
        }
        RefProblem::NotSha => {
            format!("repository URL ref `{reference}` is not a pinned commit SHA")
        }
    };
    span_lint_hir_and_then(
        lint_context,
        UNPINNED_REPO_REF,
        hir_id,
        span,
        message,
        |diag| {
            diag.help(
                "pin the URL to a commit SHA so the linked file or directory \
             cannot change without warning",
            );
        },
    );
}
