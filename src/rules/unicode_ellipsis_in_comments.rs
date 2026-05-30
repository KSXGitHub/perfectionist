use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::def_id::LOCAL_CRATE;
use rustc_span::{BytePos, Pos, RelativeBytePos, SourceFile, Span, SyntaxContext};

use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::emit_at_enclosing_hir;
use crate::literal_scan::emit_flagged_char_hir;
use crate::module_reparse::crate_module_files;

declare_tool_lint! {
    /// ### What it does
    /// Forbids U+2026 HORIZONTAL ELLIPSIS (`…`) in regular `//` and
    /// `/* */` comments. Doc comments (`///`, `//!`) are covered by a
    /// sibling lint.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// ASCII `...` survives every encoding round-trip, every terminal,
    /// every `grep` invocation, and every `git diff` viewer without
    /// rendering as `?` or a tofu box. The Unicode form usually arrives
    /// by accident from autocorrect.
    ///
    /// ### Example
    /// **Bad:**
    /// ```rust,ignore
    /// // TODO: handle the empty-tree case…
    /// ```
    /// **Good:**
    /// ```rust,ignore
    /// // TODO: handle the empty-tree case...
    /// ```
    #[cfg_attr(
        dylint_lib = "perfectionist",
        expect(
            perfectionist::unicode_ellipsis_in_docs,
            reason = "this rule's own rustdoc names the U+2026 glyph it governs"
        )
    )]
    pub perfectionist::UNICODE_ELLIPSIS_IN_COMMENTS,
    Warn,
    "U+2026 HORIZONTAL ELLIPSIS in non-doc comments; prefer `...`",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::unicode_ellipsis_in_comments";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Extra characters to flag alongside U+2026. Useful for catching
    /// near-relatives such as U+22EF MIDLINE HORIZONTAL ELLIPSIS (`⋯`)
    /// or U+2025 TWO DOT LEADER (`‥`) that the same autocorrect
    /// pipelines occasionally insert. Empty by default.
    extra_flagged_chars: Vec<char>,
    /// Scan `//` line comments. Defaults to `true`.
    scan_line_comments: bool,
    /// Scan `/* ... */` block comments. Defaults to `true`.
    scan_block_comments: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            extra_flagged_chars: Vec::new(),
            scan_line_comments: true,
            scan_block_comments: true,
        }
    }
}

pub struct UnicodeEllipsisInComments {
    flagged_chars: Vec<char>,
    scan_line_comments: bool,
    scan_block_comments: bool,
}

impl UnicodeEllipsisInComments {
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
            scan_line_comments: config.scan_line_comments,
            scan_block_comments: config.scan_block_comments,
        }
    }
}

impl_lint_pass!(UnicodeEllipsisInComments => [UNICODE_ELLIPSIS_IN_COMMENTS]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[UNICODE_ELLIPSIS_IN_COMMENTS]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive =
        resolved_state("unicode_ellipsis_in_comments", DefaultState::Active)
    {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(UnicodeEllipsisInComments::new()));
}

impl<'tcx> LateLintPass<'tcx> for UnicodeEllipsisInComments {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        if !(self.scan_line_comments || self.scan_block_comments) {
            return;
        }
        // Only scan files the user wrote as Rust modules. A file pulled
        // in via `include_str!` / `include_bytes!` is data, not Rust, so
        // a U+2026 inside it (after a `//` the lexer reads as a comment)
        // must not be flagged. See
        // <https://github.com/KSXGitHub/perfectionist/issues/179>.
        let module_files = crate_module_files(lint_context);
        let source_map = lint_context.sess().source_map();
        let mut violations: Vec<(Span, char)> = Vec::new();
        for source_file in source_map.files().iter() {
            if source_file.cnum != LOCAL_CRATE {
                continue;
            }
            if !module_files.contains(&source_file.name) {
                continue;
            }
            let Some(source_text) = source_file.src.as_deref() else {
                continue;
            };
            let mut offset: u32 = 0;
            for token in tokenize(source_text, FrontmatterAllowed::Yes) {
                let token_len = token.len;
                let should_scan_token = match token.kind {
                    TokenKind::LineComment { doc_style: None } => self.scan_line_comments,
                    TokenKind::BlockComment {
                        doc_style: None, ..
                    } => self.scan_block_comments,
                    _ => false,
                };
                if should_scan_token {
                    let end = offset
                        .checked_add(token_len)
                        .expect("source-file offset overflowed u32");
                    let comment = &source_text[offset as usize..end as usize];
                    self.collect_comment(source_file, offset, comment, &mut violations);
                }
                offset = offset
                    .checked_add(token_len)
                    .expect("source-file offset overflowed u32");
            }
        }
        emit_at_enclosing_hir(lint_context.tcx, violations, |hir_id, span, character| {
            emit_flagged_char_hir(
                lint_context,
                UNICODE_ELLIPSIS_IN_COMMENTS,
                hir_id,
                character,
                span,
                "comment",
            );
        });
    }
}

impl UnicodeEllipsisInComments {
    fn collect_comment(
        &self,
        source_file: &SourceFile,
        comment_offset: u32,
        comment: &str,
        out: &mut Vec<(Span, char)>,
    ) {
        for (byte_index, character) in comment.char_indices() {
            if !self.flagged_chars.contains(&character) {
                continue;
            }
            let span_start = source_file.absolute_position(RelativeBytePos::from_u32(
                comment_offset + byte_index as u32,
            ));
            let span_end = BytePos::from_u32(span_start.0 + character.len_utf8() as u32);
            out.push((
                Span::new(span_start, span_end, SyntaxContext::root(), None),
                character,
            ));
        }
    }
}
