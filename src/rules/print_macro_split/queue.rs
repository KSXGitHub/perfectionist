//! The pre-expansion pass parks one of these per violation for the
//! late pass to emit, after `cfg_attr` has resolved, at the deepest
//! enclosing HIR node.
//!
//! Unlike the span-only payloads of `macro_trailing_comma` and
//! `macro_argument_binding`, the rewrite is computed up front (it needs
//! the source map, available in the pre-expansion pass) and carried as
//! a ready `String`. [`Span`] is `Copy + Send + Sync` and `String` is
//! `Send`, so the queue is safe to hold in a process-wide static.

use rustc_span::Span;

pub(super) struct PendingViolation {
    /// Span of the whole macro invocation — both the diagnostic anchor
    /// and the region the suggestion replaces.
    pub(super) call_span: Span,
    /// The wrapped, continuation-folded replacement for `call_span`.
    pub(super) replacement: String,
}
