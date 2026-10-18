//! Late-pass machinery: scans the crate's comments and the panic
//! messages of `expect` / `expect_err`, drains the pre-expansion
//! pass's queue of macro-argument findings, and emits every dash at
//! its enclosing HIR node so that a per-item, per-field or
//! per-module `#[allow]` / `#[expect]` resolves. The anchoring walk is
//! [`crate::enclosing_hir::emit_at_enclosing_hir`].

use super::blockquote::scan_block_quotations;
use super::config::{ResolvedConfig, resolved_config};
use super::emit::emit_dash;
use super::finding::{Finding, Surface};
use super::literal::scan_string_literal;
use super::scan::scan_dashes;
use super::{MESSAGE_METHODS, PENDING_VIOLATIONS};
use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::receiver_is_option_or_result;
use crate::enclosing_hir::emit_at_enclosing_hir;
use crate::markdown::scan_code_regions;
use rustc_ast::LitKind;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use std::sync::PoisonError;

pub(super) struct EmDashProseLate {
    config: ResolvedConfig,
    /// Dashes found in an `expect` / `expect_err` message, collected
    /// as the HIR walk reaches each call and emitted once the walk is
    /// over, so that they interleave with the other surfaces' findings
    /// by position rather than arriving ahead of them.
    found: Vec<(Span, Finding)>,
}

impl EmDashProseLate {
    pub(super) fn new() -> Self {
        Self {
            config: resolved_config(),
            found: Vec::new(),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for EmDashProseLate {
    fn check_expr(&mut self, lint_context: &LateContext<'tcx>, expr: &Expr<'tcx>) {
        if !self.config.scan_macro_messages {
            return;
        }
        let ExprKind::MethodCall(path_segment, receiver, arguments, _) = expr.kind else {
            return;
        };
        let method = path_segment.ident.name.as_str();
        if !MESSAGE_METHODS.contains(&method)
            || !receiver_is_option_or_result(lint_context, receiver)
        {
            return;
        }
        let Some(ExprKind::Lit(literal)) = arguments.first().map(|argument| argument.kind) else {
            return;
        };
        if !matches!(literal.node, LitKind::Str(..)) {
            return;
        }
        // The dash span is narrower than the node that produced it,
        // which is the shape `report_in_external_macro: false` cannot
        // filter. No extra guard is needed all the same: a dash is
        // reported only where the span's own source text is a string
        // literal, so a derive that stamps a synthesised `expect` over
        // a user-source span is reported on the literal the user
        // actually wrote there, and one that stamps it over anything
        // else fails `scan_string_literal`'s read of the snippet.
        let Ok(snippet) = lint_context
            .sess()
            .source_map()
            .span_to_snippet(literal.span)
        else {
            return;
        };
        for (span, dash) in scan_string_literal(&snippet, literal.span, self.config.policy) {
            self.found.push((
                span,
                Finding {
                    dash,
                    surface: Surface::MethodMessage(method.to_owned()),
                },
            ));
        }
    }

    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let mut findings = self.collect_comments(lint_context);
        findings.append(&mut self.found);
        findings.append(&mut drain_queue());
        // By position, so the two scans' findings interleave in the
        // order the reader scans the file rather than by which scan
        // produced them.
        findings.sort_by_key(|(span, _)| span.lo());
        let message = self.config.message.as_deref();
        emit_at_enclosing_hir(lint_context.tcx, findings, |hir_id, span, finding| {
            emit_dash(lint_context, hir_id, span, finding, message);
        });
    }
}

impl EmDashProseLate {
    /// Every dash in a comment of the crate's own module source files.
    fn collect_comments(&self, lint_context: &LateContext<'_>) -> Vec<(Span, Finding)> {
        let mut findings: Vec<(Span, Finding)> = Vec::new();
        if !(self.config.scan_doc_comments || self.config.scan_regular_comments) {
            return findings;
        }
        walk_local_comments(lint_context, |chunk| {
            self.collect_chunk(chunk, &mut findings);
        });
        findings
    }

    fn collect_chunk(&self, chunk: &CommentChunk<'_>, out: &mut Vec<(Span, Finding)>) {
        let (is_doc, surface, scanned) = match chunk.surface {
            CommentSurface::DocBlock | CommentSurface::DocBlockBlock => {
                (true, Surface::DocComment, self.config.scan_doc_comments)
            }
            CommentSurface::PlainLine | CommentSurface::PlainBlock => {
                (false, Surface::Comment, self.config.scan_regular_comments)
            }
        };
        if !scanned {
            return;
        }
        // Code spans join code blocks in the skip mask. A dash inside
        // either is part of the code being quoted, in a plain comment
        // as much as in a doc comment, so both comment surfaces read
        // the markup the same way.
        let mut skips = scan_code_regions(&chunk.rendered, true);
        // A blockquote is a markdown construct, so only the doc
        // surface has one to find. `position_in_skip` tests each range
        // on its own, so appending to the mask needs no merge.
        if is_doc {
            skips.append(&mut scan_block_quotations(&chunk.rendered));
        }
        for (range, dash) in scan_dashes(&chunk.rendered, &skips, self.config.policy) {
            // A range that crosses the synthesised `\n` between two
            // joined comment lines has no single-line span to point
            // at. `scan_dashes` requires a space on each side of a
            // hyphen run and every Unicode dash is one character, so
            // nothing it reports crosses a line; the guard is
            // defensive.
            let Some(span) = chunk.span_for_range(range) else {
                continue;
            };
            out.push((
                span,
                Finding {
                    dash,
                    surface: surface.clone(),
                },
            ));
        }
    }
}

/// Take everything the pre-expansion pass parked.
fn drain_queue() -> Vec<(Span, Finding)> {
    let mut guard = PENDING_VIOLATIONS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    core::mem::take(&mut *guard)
}
