//! Counting lines of code in a stretch of Rust source, for the rules
//! that cap the length of a function body or a file.

use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};

/// How many lines of `source` hold a token that is neither whitespace
/// nor a comment.
pub(crate) fn count_code_lines(source: &str) -> usize {
    let mut code_lines = 0;
    let mut line_has_code = false;
    let mut offset = 0;
    for token in tokenize(source, FrontmatterAllowed::No) {
        let text = &source[offset..offset + token.len as usize];
        offset += token.len as usize;
        let is_code = !matches!(
            token.kind,
            TokenKind::Whitespace | TokenKind::LineComment { .. } | TokenKind::BlockComment { .. },
        );
        // Each newline inside the token ends a line. The first ends the
        // line the token started on, which counts if anything before it
        // was code; the rest end lines lying wholly inside the token,
        // which count only for a code token such as a multi-line string.
        for (newline_index, _) in text.match_indices('\n').enumerate() {
            if is_code || (newline_index == 0 && line_has_code) {
                code_lines += 1;
            }
        }
        line_has_code = if text.contains('\n') {
            is_code
        } else {
            line_has_code || is_code
        };
    }
    if line_has_code {
        code_lines += 1;
    }
    code_lines
}

#[cfg(test)]
mod tests;
