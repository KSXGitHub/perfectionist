//! Per-attribute trigger evaluation and diagnostic emission. The
//! `check_attribute` driver (in the parent module) hands off here
//! once an attribute looks like a candidate (lint-level name or
//! `cfg_attr` trace inheritance already resolved).

use super::insertion::{build_reason_insertion, escape_for_rust_string};
use super::scan::{Comment, find_trailing_comment};
use super::{LINT_ATTRIBUTE_TRAILING_COMMENT, LintAttributeTrailingComment};
use crate::common::attr_has_reason;
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_ast::MetaItemInner;
use rustc_errors::Applicability;
use rustc_lint::{EarlyContext, LintContext};
use rustc_span::{BytePos, Pos, RelativeBytePos, SourceFile, Span};

impl LintAttributeTrailingComment {
    /// Apply the trigger and emission for one lint-level invocation.
    ///
    /// `outer_span` is the syntactic attribute's span (the one whose
    /// closing `]` the trailing comment follows). `invocation_span`
    /// is the inner `allow(...)` / `expect(...)` / `warn(...)` /
    /// `deny(...)` / `forbid(...)` meta-item's span — the insertion
    /// target for the lifted `reason` field. For a bare
    /// `#[allow(...)]` the two are the same.
    ///
    /// Returns `true` iff the rule actually emitted a diagnostic
    /// (used by `check_attribute` to decide whether to consume the
    /// pending `cfg_attr_trace` outer span).
    pub(super) fn check(
        &self,
        lint_context: &EarlyContext<'_>,
        outer_span: Span,
        invocation_span: Span,
        args: &[MetaItemInner],
    ) -> bool {
        if attr_has_reason(args).is_some() {
            return false;
        }

        let source_map = lint_context.sess().source_map();
        let source_file = source_map.lookup_source_file(outer_span.lo());
        let Some(source_text) = source_file.src.as_deref() else {
            return false;
        };
        let file_start = source_file.start_pos;
        // `BytePos`es from a span living inside this source file are
        // always `>= start_pos`; the `checked_sub` is defensive only.
        let Some(outer_hi) = outer_span.hi().0.checked_sub(file_start.0) else {
            return false;
        };
        let outer_hi = outer_hi as usize;
        if outer_hi > source_text.len() {
            return false;
        }

        // `find_trailing_comment` filters out empty-normalised
        // matches (a bare `//` or an all-decoration divider), so an
        // unhelpful trailing comment does not produce a vacuous
        // `reason = ""`.
        let Some(comment) = find_trailing_comment(source_text, outer_hi) else {
            return false;
        };
        self.emit(lint_context, &source_file, invocation_span, &comment);
        true
    }

    fn emit(
        &self,
        lint_context: &EarlyContext<'_>,
        source_file: &SourceFile,
        invocation_span: Span,
        comment: &Comment,
    ) {
        let snippet = match lint_context
            .sess()
            .source_map()
            .span_to_snippet(invocation_span)
        {
            Ok(snippet) => snippet,
            Err(_) => return,
        };
        let escaped = escape_for_rust_string(&comment.text);
        let Some(insertion) = build_reason_insertion(&snippet, &escaped) else {
            return;
        };

        let arg_lo = invocation_span.lo() + BytePos::from_usize(insertion.start);
        let arg_hi = invocation_span.lo() + BytePos::from_usize(insertion.end);
        let arg_span = invocation_span.with_lo(arg_lo).with_hi(arg_hi);

        let diag_span = file_span(
            source_file,
            comment.diag_start,
            comment.diag_end,
            invocation_span,
        );
        let delete_span = file_span(
            source_file,
            comment.delete_start,
            comment.delete_end,
            invocation_span,
        );

        span_lint_and_then(
            lint_context,
            LINT_ATTRIBUTE_TRAILING_COMMENT,
            diag_span,
            "comment can be lifted into the attribute's `reason` field",
            |diagnostic| {
                diagnostic.multipart_suggestion(
                    "lift the comment into a `reason` field",
                    vec![
                        (arg_span, insertion.replacement),
                        (delete_span, String::new()),
                    ],
                    // The comment's attachment to the attribute is
                    // unambiguous, but its prose was written as a
                    // margin note, not as a `reason` value — the
                    // author may want to reword it before committing.
                    // `MaybeIncorrect` offers the rewrite without
                    // letting `cargo fix` apply it unreviewed.
                    Applicability::MaybeIncorrect,
                );
            },
        );
    }
}

/// Build an absolute [`Span`] covering `[start, end)` byte offsets
/// inside `source_file`. Inherits its [`rustc_span::SyntaxContext`]
/// (and any `LocalDefId` parent) from `anchor` — typically the
/// attribute's own span — so the resulting span participates in
/// `report_in_external_macro: false` filtering and aligns with the
/// other suggestion spans rather than collapsing to root.
fn file_span(source_file: &SourceFile, start: usize, end: usize, anchor: Span) -> Span {
    let lo = source_file.absolute_position(RelativeBytePos::from_usize(start));
    let hi = source_file.absolute_position(RelativeBytePos::from_usize(end));
    Span::new(lo, hi, anchor.ctxt(), anchor.parent())
}
