use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::emit_at_enclosing_hir;
use crate::literal_scan::emit_flagged_char_hir;
use crate::markdown::{position_in_skip, scan_code_regions};

declare_tool_lint! {
    /// ### What it does
    /// Forbids U+2026 HORIZONTAL ELLIPSIS (`…`) in doc comments —
    /// `///` and `//!` line forms and the `/** */` / `/*! */` block
    /// forms. Prefer the three-ASCII-dot form `...`. Regular `//` and
    /// `/* */` comments are covered by a sibling lint
    /// (`perfectionist::unicode_ellipsis_in_comments`).
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// ASCII `...` survives every encoding round-trip, every terminal,
    /// every copy-paste, every `grep` invocation, and every `git diff`
    /// viewer without rendering as `?` or a tofu box. The visual
    /// difference between `…` and `...` is small enough that the
    /// Unicode form usually arrives by accident — autocorrect, an IDE
    /// smart-quote setting — rather than as a deliberate choice in
    /// technical writing.
    ///
    /// ### Example
    /// ```rust,ignore
    /// /// Walk the tree, collecting sizes…
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// /// Walk the tree, collecting sizes...
    /// ```
    pub perfectionist::UNICODE_ELLIPSIS_IN_DOCS,
    Warn,
    "U+2026 HORIZONTAL ELLIPSIS in doc comments; prefer `...`",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::unicode_ellipsis_in_docs";

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Extra characters to flag alongside U+2026. Useful for catching
    /// near-relatives such as U+22EF MIDLINE HORIZONTAL ELLIPSIS (`⋯`)
    /// or U+2025 TWO DOT LEADER (`‥`) that the same autocorrect
    /// pipelines occasionally insert. Empty by default.
    extra_flagged_chars: Vec<char>,
    /// Whether to also flag a character inside an inline code span
    /// (`` `…` ``). Defaults to `false`: code spans often quote example
    /// text where the ellipsis is meaningful, so they are left alone
    /// unless this is set to `true`. Code *blocks* — fenced
    /// (` ``` … ``` `), `~~~`-fenced, four-space indented, and the
    /// doc-test code they hold — are always skipped regardless of this
    /// knob.
    scan_code_spans: bool,
}

pub struct UnicodeEllipsisInDocs {
    flagged_chars: Vec<char>,
    scan_code_spans: bool,
}

impl UnicodeEllipsisInDocs {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let mut flagged_chars = vec!['\u{2026}'];
        for character in config.extra_flagged_chars {
            if !flagged_chars.contains(&character) {
                flagged_chars.push(character);
            }
        }
        Self {
            flagged_chars,
            scan_code_spans: config.scan_code_spans,
        }
    }
}

impl_lint_pass!(UnicodeEllipsisInDocs => [UNICODE_ELLIPSIS_IN_DOCS]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[UNICODE_ELLIPSIS_IN_DOCS]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("unicode_ellipsis_in_docs", DefaultState::Active)
    {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(UnicodeEllipsisInDocs::new()));
}

impl<'tcx> LateLintPass<'tcx> for UnicodeEllipsisInDocs {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let mut violations: Vec<(Span, char)> = Vec::new();
        walk_local_comments(lint_context.sess(), |chunk| match chunk.surface {
            CommentSurface::DocBlock | CommentSurface::DocBlockBlock => {
                self.collect_doc_chunk(chunk, &mut violations);
            }
            CommentSurface::PlainLine | CommentSurface::PlainBlock => {}
        });
        emit_at_enclosing_hir(lint_context.tcx, violations, |hir_id, span, character| {
            emit_flagged_char_hir(
                lint_context,
                UNICODE_ELLIPSIS_IN_DOCS,
                hir_id,
                character,
                span,
                "doc comment",
            );
        });
    }
}

impl UnicodeEllipsisInDocs {
    fn collect_doc_chunk(&self, chunk: &CommentChunk<'_>, out: &mut Vec<(Span, char)>) {
        // Code spans join the skip mask unless the user opts into
        // scanning them; code blocks are always masked.
        let skips = scan_code_regions(&chunk.rendered, !self.scan_code_spans);
        for (byte_offset, character) in chunk.rendered.char_indices() {
            if !self.flagged_chars.contains(&character) {
                continue;
            }
            if position_in_skip(&skips, byte_offset) {
                continue;
            }
            // A flagged character always lands inside a content line —
            // never the synthesised `\n` between joined `///` lines —
            // so `span_for` returns `Some`; the guard is defensive.
            let Some(span) = chunk.span_for(byte_offset, character.len_utf8() as u32) else {
                continue;
            };
            out.push((span, character));
        }
    }
}
