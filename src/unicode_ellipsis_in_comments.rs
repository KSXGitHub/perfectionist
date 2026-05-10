use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_sugg;
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

const CONFIG_KEY: &str = "unicode_ellipsis_in_comments";

#[derive(Debug, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {
    also_flag: Vec<String>,
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
    needles: Vec<String>,
    scopes: BTreeSet<Scope>,
}

impl UnicodeEllipsisInComments {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let mut needles = vec!["\u{2026}".to_owned()];
        for extra in config.also_flag {
            if !needles.contains(&extra) {
                needles.push(extra);
            }
        }
        Self {
            needles,
            scopes: config.scope.into_iter().collect(),
        }
    }
}

impl_lint_pass!(UnicodeEllipsisInComments => [UNICODE_ELLIPSIS_IN_COMMENTS]);

pub fn register(lint_store: &mut LintStore) {
    lint_store.register_lints(&[UNICODE_ELLIPSIS_IN_COMMENTS]);
    lint_store.register_early_pass(|| Box::new(UnicodeEllipsisInComments::new()));
}

impl EarlyLintPass for UnicodeEllipsisInComments {
    fn check_crate(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        let source_map = cx.sess().source_map();
        let want_line = self.scopes.contains(&Scope::Line);
        let want_block = self.scopes.contains(&Scope::Block);
        if !(want_line || want_block) {
            return;
        }
        for source_file in source_map.files().iter() {
            if source_file.cnum != LOCAL_CRATE {
                continue;
            }
            let Some(src) = source_file.src.as_deref() else {
                continue;
            };
            let mut offset: u32 = 0;
            for token in tokenize(src, FrontmatterAllowed::Yes) {
                let len = token.len;
                let scan = match token.kind {
                    TokenKind::LineComment { doc_style: None } => want_line,
                    TokenKind::BlockComment {
                        doc_style: None, ..
                    } => want_block,
                    _ => false,
                };
                if scan {
                    let end = offset
                        .checked_add(len)
                        .expect("source-file offset overflowed u32");
                    let comment = &src[offset as usize..end as usize];
                    self.scan_comment(cx, source_file, offset, comment);
                }
                offset = offset
                    .checked_add(len)
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
        for (idx, ch) in comment.char_indices() {
            let mut buf = [0u8; 4];
            let ch_str = ch.encode_utf8(&mut buf);
            let Some(needle) = self.needles.iter().find(|n| n.as_str() == ch_str) else {
                continue;
            };
            let char_len = ch.len_utf8() as u32;
            let lo = source_file
                .absolute_position(RelativeBytePos::from_u32(comment_offset + idx as u32));
            let hi = BytePos::from_u32(lo.0 + char_len);
            let span = Span::new(lo, hi, SyntaxContext::root(), None);
            let applicability = if needle == "\u{2026}" {
                Applicability::MachineApplicable
            } else {
                Applicability::MaybeIncorrect
            };
            span_lint_and_sugg(
                cx,
                UNICODE_ELLIPSIS_IN_COMMENTS,
                span,
                format!("Unicode `{ch}` (U+{:04X}) in comment", ch as u32),
                "use ASCII `...` instead",
                "...".to_owned(),
                applicability,
            );
        }
    }
}
