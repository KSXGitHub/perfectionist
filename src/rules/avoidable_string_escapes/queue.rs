//! The pre-expansion pass parks one of these per macro-embedded string
//! literal it rewrites, for the late pass to dedup and emit after
//! `cfg_attr` has resolved, at the deepest enclosing HIR node.
//!
//! As with `unsplit_print_macro`, the rewrite is computed up front (it
//! needs the source map, available pre-expansion) and carried as a
//! ready `String`. `Span` is `Copy + Send + Sync` and `String` is
//! `Send`, so the queue is safe to hold in a process-wide static.

use rustc_span::Span;

pub(super) struct PendingViolation {
    /// Span of the string literal — both the diagnostic anchor and the
    /// region the suggestion replaces.
    pub(super) span: Span,
    /// The raw-string replacement for `span`.
    pub(super) suggestion: String,
}
