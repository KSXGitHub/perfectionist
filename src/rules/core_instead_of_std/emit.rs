//! Judging the paths written through one `core` / `alloc` token, and
//! emitting what they earn.
//!
//! The unit of decision is the token, not the path. `use core::fmt;`
//! and `use core::{fmt::Display, ops::Add};` reach the pass as one, two,
//! or more paths — HIR splits a brace list into a `Use` item per leaf,
//! and visits each leaf once per namespace its name resolves in — but
//! all of them are written through a single `core` in the source, and
//! that is the only token the fix touches. So the paths sharing a token
//! are collected into a [`Group`] and answered together: every one of
//! them reachable through `std` earns the rewrite, and a single
//! [`Point::Blocked`] withdraws it for the whole group, leaving the
//! reachable ones with a `help` apiece.

use super::CORE_INSTEAD_OF_STD;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir::HirId;
use rustc_lint::LateContext;
use rustc_span::{Span, Symbol};

/// One path written through a group's crate token.
pub(super) enum Point {
    /// The path names an item that `std` reaches by the same suffix, so
    /// swapping the crate token moves it without changing what it means.
    /// The span is the path's last segment — where the `help` lands when
    /// the group as a whole cannot be rewritten — and the [`HirId`] is
    /// the node the path belongs to, so that an `#[allow(...)]` around
    /// it still applies however late the group is flushed.
    Rewritable { hir_id: HirId, span: Span },
    /// The path has to keep its crate token: `std` does not reach the
    /// item by this suffix, or `skip_paths` exempts it. Nothing is
    /// reported for the path itself — it is not a violation — but it
    /// vetoes the rewrite of the token it shares with its siblings.
    Blocked,
}

/// The paths written through one `core` / `alloc` token.
pub(super) struct Group {
    /// The token itself: the group's identity, and the span the
    /// suggestion replaces.
    pub(super) crate_span: Span,
    /// What the token spells, for the diagnostic message.
    pub(super) crate_name: Symbol,
    pub(super) points: Vec<Point>,
}

/// Report a finished group, if there was one. A group with no
/// [`Point::Rewritable`] holds nothing to report: every path in it is
/// one the rule leaves alone.
pub(super) fn flush(cx: &LateContext<'_>, group: Option<Group>) {
    let Some(Group {
        crate_span,
        crate_name,
        points,
    }) = group
    else {
        return;
    };
    let blocked = points.iter().any(|point| matches!(point, Point::Blocked));
    let mut rewritable = points.iter().filter_map(|point| match point {
        Point::Rewritable { hir_id, span } => Some((*hir_id, *span)),
        Point::Blocked => None,
    });
    let message = format!("item named through `{crate_name}` instead of `std`");

    if !blocked {
        let Some((hir_id, _)) = rewritable.next() else {
            return;
        };
        // One suggestion for the whole group: every path through this
        // token moves with it, so re-reporting per path would be the
        // same fix over the same span several times over.
        span_lint_hir_and_then(
            cx,
            CORE_INSTEAD_OF_STD,
            hir_id,
            crate_span,
            message,
            |diag| {
                diag.span_suggestion(
                    crate_span,
                    "name the item through `std`",
                    "std",
                    Applicability::MachineApplicable,
                );
            },
        );
        return;
    }

    let help = format!(
        "name this item through `std`: the `{crate_name}` segment is shared with a \
         name that has to keep it, so it cannot be rewritten in place",
    );
    for (hir_id, span) in rewritable {
        span_lint_hir_and_then(
            cx,
            CORE_INSTEAD_OF_STD,
            hir_id,
            span,
            message.clone(),
            |diag| {
                diag.help(help.clone());
            },
        );
    }
}
