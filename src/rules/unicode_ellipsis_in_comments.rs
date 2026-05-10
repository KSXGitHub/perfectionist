use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_ast::Crate;
use rustc_errors::Applicability;
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{
    BytePos, Pos, RelativeBytePos, SourceFile, Span, SyntaxContext, def_id::LOCAL_CRATE,
};

declare_tool_lint! {
    /// ### What it does
    /// Forbids U+2026 HORIZONTAL ELLIPSIS (`…`) in regular `//` and
    /// `/* */` comments. Doc comments (`///`, `//!`) are covered by a
    /// sibling lint.
    ///
    /// ### Why is this bad?
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
#[serde(default, rename_all = "snake_case")]
struct Config {
    also_flag: Vec<char>,
    scope: Vec<Scope>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            also_flag: Vec::new(),
            scope: vec![Scope::Line, Scope::Block],
        }
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Scope {
    Line,
    Block,
}

pub struct UnicodeEllipsisInComments {
    flagged_chars: Vec<char>,
    scopes: BTreeSet<Scope>,
}

impl UnicodeEllipsisInComments {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let mut flagged_chars = vec!['\u{2026}'];
        for ch in config.also_flag {
            if !flagged_chars.contains(&ch) {
                flagged_chars.push(ch);
            }
        }
        Self {
            flagged_chars,
            scopes: config.scope.into_iter().collect(),
        }
    }
}

impl_lint_pass!(UnicodeEllipsisInComments => [UNICODE_ELLIPSIS_IN_COMMENTS]);

/// Register this rule's lint declaration. Paired with [`register_pass`];
/// see the module-level convention documented in `register_lints`.
pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[UNICODE_ELLIPSIS_IN_COMMENTS]);
}

/// Install this rule's early pass.
pub fn register_pass(lint_store: &mut LintStore) {
    lint_store.register_early_pass(|| Box::new(UnicodeEllipsisInComments::new()));
}

impl EarlyLintPass for UnicodeEllipsisInComments {
    fn check_crate(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        let source_map = cx.sess().source_map();
        let scan_line_comments = self.scopes.contains(&Scope::Line);
        let scan_block_comments = self.scopes.contains(&Scope::Block);
        if !(scan_line_comments || scan_block_comments) {
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
                    TokenKind::LineComment { doc_style: None } => scan_line_comments,
                    TokenKind::BlockComment {
                        doc_style: None, ..
                    } => scan_block_comments,
                    _ => false,
                };
                if should_scan_token {
                    let end = offset
                        .checked_add(token_len)
                        .expect("source-file offset overflowed u32");
                    let comment = &source_text[offset as usize..end as usize];
                    self.scan_comment(cx, source_file, offset, comment);
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
        cx: &EarlyContext<'_>,
        source_file: &SourceFile,
        comment_offset: u32,
        comment: &str,
    ) {
        for (byte_index, ch) in comment.char_indices() {
            if !self.flagged_chars.contains(&ch) {
                continue;
            }
            let char_len = ch.len_utf8() as u32;
            let span_start = source_file.absolute_position(RelativeBytePos::from_u32(
                comment_offset + byte_index as u32,
            ));
            let span_end = BytePos::from_u32(span_start.0 + char_len);
            let span = Span::new(span_start, span_end, SyntaxContext::root(), None);
            let applicability = if ch == '\u{2026}' {
                Applicability::MachineApplicable
            } else {
                Applicability::MaybeIncorrect
            };
            span_lint_and_then(
                cx,
                UNICODE_ELLIPSIS_IN_COMMENTS,
                span,
                format!("Unicode `{ch}` (U+{:04X}) in comment", ch as u32),
                |diagnostic| {
                    diagnostic.span_suggestion(
                        span,
                        "use ASCII `...` instead",
                        "...".to_owned(),
                        applicability,
                    );
                },
            );
        }
    }
}
