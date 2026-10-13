//! Deciding whether a body hands over a borrow of `self`'s own data
//! rather than computing one.
//!
//! The written form answers it, and a `Deref` on the way is taken to be
//! free whatever the implementation does. One that costs something is
//! an anti-pattern of its own, not a case for this rule to work around;
//! and the std type that comes closest, a `LazyLock`, pays for its
//! initializer once, where the cost this rule measures is the one a
//! caller meets on every call.
//!
//! What the *callee* of an `as_*` call costs is not read either. The
//! guidelines give that prefix the same meaning this rule enforces, so
//! trusting it here is trusting what the catalogue already holds
//! authors to.

use rustc_hir::{BorrowKind, Expr, ExprKind, QPath, UnOp};
use rustc_span::kw;

/// The prefix a method call in the body may be trusted to be free
/// under, and the one a flagged method should have worn instead: the
/// guidelines give both the same meaning.
pub(super) const AS_PREFIX: &str = "as_";

/// Whether the body hands over a borrow it already had: a place inside
/// `self`, or a single `as_*` call on one. Anything else is left alone,
/// a free body among them, since what an arbitrary body costs is not a
/// question this can answer.
pub(super) fn costless_body(expr: &Expr<'_>) -> bool {
    if let ExprKind::MethodCall(segment, receiver, [], _) = expr.kind {
        return segment.ident.name.as_str().starts_with(AS_PREFIX) && borrows_self(receiver);
    }
    borrows_self(expr)
}

/// Whether `expr` names a place inside `self`, or borrows one:
/// `self.name`, `&self.name`, `&self.inner.name`, `&*self.boxed`.
fn borrows_self(mut expr: &Expr<'_>) -> bool {
    loop {
        expr = match expr.kind {
            ExprKind::AddrOf(BorrowKind::Ref, _, inner) => inner,
            ExprKind::Field(base, _) => base,
            ExprKind::Unary(UnOp::Deref, base) => base,
            ExprKind::Path(QPath::Resolved(None, path)) => {
                return matches!(path.segments, [segment] if segment.ident.name == kw::SelfLower);
            }
            _ => return false,
        };
    }
}
