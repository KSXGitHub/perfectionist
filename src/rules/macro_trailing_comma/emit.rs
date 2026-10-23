//! Diagnostic emission for the two violation shapes.
//!
//! Both suggestions are `MaybeIncorrect`, so `cargo dylint --fix` and
//! an editor's fix-all leave them alone and a human applies them. The
//! reason is that rustfmt reformats a macro invocation whose
//! arguments parse as an expression list, and does it on terms of its
//! own.
//!
//! Which comma rustfmt will respell turns on the invocation's name and
//! nesting rather than on its delimiters. A top-level `vec!` is the one
//! invocation rustfmt rewrites as an array literal, normalising the
//! trailing comma to the same vertical policy this lint enforces.
//! Every other invocation, `vec!` nested inside another macro
//! included, keeps whatever trailing comma it was written with, which
//! turns the insertion into churn in two ways:
//!
//! * An inserted comma survives the collapse onto one line:
//!
//!   ```text
//!   format!(          format!(           format!("{} {}", 1, 2,)
//!       "{} {}",          "{} {}",
//!       1,        -->     1,         -->
//!       2                 2,
//!   )                 )
//!     as written        comma inserted     rustfmt's output
//!   ```
//!
//!   and this same lint reports `format!("{} {}", 1, 2,)` in the
//!   opposite direction, as a single-line invocation that should not
//!   end with a trailing comma. Measured, the same exchange runs for
//!   `println!`, `assert_eq!` and any bracket-delimited list macro an
//!   `extra_macros` entry makes eligible.
//! * An invocation whose sole element is multi-line settles somewhere
//!   rustfmt would not have put it. Reshaping to the compact layout is
//!   what rustfmt does with such an element, and the inserted comma
//!   rides along to the closing line, so a block-indented
//!   `my_list![Inner { field: 1 }]` fixed to carry one reformats to:
//!
//!   ```text
//!   my_list![Inner {
//!       field: 1,
//!   },]
//!   ```
//!
//!   The `},]` is a resting state: rustfmt leaves it, and the
//!   suppression that skips the compact layout keeps this lint quiet
//!   about it. A `vec!` is spared only because the array path strips
//!   that comma.
//!
//! Neither disagreement runs forever: rustfmt is idempotent, and the
//! second fix lands on a form rustfmt leaves alone. One round of
//! reverse-direction churn is still more than a silent fixer should
//! spend, which is what the applicability buys back.
//!
//! The removal is held to review for symmetry rather than because it
//! moves anything. rustfmt picks a macro invocation's layout from the
//! argument widths alone, with the trailing comma excluded, so deleting
//! the comma cannot turn a single-line invocation into a multi-line
//! one. What the removal mostly fires on is the shape the insertion
//! produces, and a reader seeing one half of that exchange applied
//! silently and the other half reported has to reconstruct why.

use super::MACRO_TRAILING_COMMA;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::LateContext;
use rustc_span::Span;

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
                Applicability::MaybeIncorrect,
            );
        },
    );
}

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
                Applicability::MaybeIncorrect,
            );
        },
    );
}
