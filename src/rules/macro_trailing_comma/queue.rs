//! The pre-expansion pass parks violation spans here for the late pass
//! to anchor in the HIR. The two variants distinguish the two shapes
//! of fix this rule offers: insert a missing trailing comma, or remove
//! a gratuitous one.

use rustc_span::Span;

#[derive(Debug, Clone, Copy)]
pub(super) enum PendingViolation {
    /// Multi-line macro invocation, missing a trailing comma after the
    /// last argument. The span is the insertion point.
    Insert(Span),
    /// Single-line macro invocation, with a gratuitous trailing comma
    /// after the last argument. The span covers the comma to remove.
    Remove(Span),
}

impl PendingViolation {
    pub(super) fn span(self) -> Span {
        match self {
            PendingViolation::Insert(span) | PendingViolation::Remove(span) => span,
        }
    }
}
