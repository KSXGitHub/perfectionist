use rustc_ast::Crate;
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{
    BytePos, Pos, RelativeBytePos, SourceFile, Span, SyntaxContext, def_id::LOCAL_CRATE,
};

use crate::common::{DefaultState, resolved_state};
use crate::literal_scan::emit_flagged_chars;

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
    /// ```rust,ignore
    /// // TODO: handle the empty-tree case…
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// // TODO: handle the empty-tree case...
    /// ```
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
    lint_store.register_early_pass(|| Box::new(UnicodeEllipsisInComments::new()));
}

impl EarlyLintPass for UnicodeEllipsisInComments {
    fn check_crate(&mut self, lint_context: &EarlyContext<'_>, _: &Crate) {
        let source_map = lint_context.sess().source_map();
        if !(self.scan_line_comments || self.scan_block_comments) {
            return;
        }
        for source_file in source_map.files().iter() {
            if source_file.cnum != LOCAL_CRATE {
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
                    self.scan_comment(lint_context, source_file, offset, comment);
                }
                offset = offset
                    .checked_add(token_len)
                    .expect("source-file offset overflowed u32");
            }
        }
    }
}

impl UnicodeEllipsisInComments {
    fn scan_comment(
        &self,
        lint_context: &EarlyContext<'_>,
        source_file: &SourceFile,
        comment_offset: u32,
        comment: &str,
    ) {
        emit_flagged_chars(
            lint_context,
            UNICODE_ELLIPSIS_IN_COMMENTS,
            comment,
            &self.flagged_chars,
            "comment",
            |byte_index, char_len| {
                let span_start = source_file.absolute_position(RelativeBytePos::from_u32(
                    comment_offset + byte_index as u32,
                ));
                let span_end = BytePos::from_u32(span_start.0 + char_len);
                Span::new(span_start, span_end, SyntaxContext::root(), None)
            },
        );
    }
}
