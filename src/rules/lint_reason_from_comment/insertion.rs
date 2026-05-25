//! Construction of the args-list edit and the Rust-string-literal
//! escape that produce the `reason = "<text>"` insertion.

use std::fmt::Write as _;

use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};

/// The text edit produced for the args-list insertion, expressed
/// as byte offsets inside the meta-item's source snippet. Mirrors
/// the `Insertion` shape used by `lint_silence_reason`, but the
/// `reason` literal carries the lifted comment's text rather than
/// an empty placeholder.
pub(super) struct Insertion {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) replacement: String,
}

/// Compute the byte-offset edit that inserts
/// `reason = "<escaped>"` into the attribute's argument list. Two
/// strategies, chosen by whether the closing `)` sits on its own
/// line:
///
/// - **Inline** (`)` shares its line with the last argument — both
///   `allow(foo)` and the wrapped `allow(\n    foo)`): splice before
///   the closing `)`. `, reason = "<escaped>"` when the last arg has
///   no trailing comma, ` reason = "<escaped>",` when it does, and a
///   bare `reason = "<escaped>"` into an empty `allow()`.
/// - **New line** (`)` alone on its own line, `allow(\n    foo,\n)`):
///   insert a `reason = "<escaped>",` line immediately before the
///   `)` line, matching the indentation of the preceding argument.
///   If that argument lacks a trailing comma, one is added.
///
/// Returns `None` if the snippet does not contain the expected
/// `(...)` layout (e.g. macro-expanded sources where
/// `span_to_snippet` returns a synthetic placeholder).
pub(super) fn build_reason_insertion(snippet: &str, escaped: &str) -> Option<Insertion> {
    let (open_paren_offset, close_paren_offset) = locate_outermost_parens(snippet)?;
    let head = &snippet[..close_paren_offset];

    // Pick the insertion strategy by whether the closing `)` sits on
    // its own line, *not* by whether the argument list contains a
    // newline anywhere. A true single-line list (`allow(foo)`) and a
    // wrapped list whose `)` rides the last argument's line
    // (`allow(\n    foo)`) both want the inline splice — only a `)`
    // alone on its own line (`allow(\n    foo,\n)`) wants a new
    // indented `reason` line. Keying off "any newline in the parens"
    // mis-handled the wrapped case, inserting an unindented line in
    // the wrong place.
    let close_line_start = head.rfind('\n').map_or(0, |index| index + 1);
    let close_on_own_line = head[close_line_start..]
        .trim_matches([' ', '\t', '\r'])
        .is_empty();

    if !close_on_own_line {
        let trimmed = head.trim_end_matches([' ', '\t', '\r']);
        // An empty argument list (`#[allow()]`) and a trailing-comma
        // list collapse into the same edit shape if we look only at
        // the last non-space character — the comma after a single
        // arg, or the `(` for the empty list. Distinguishing them
        // matters: with no comma at all, we need to add one.
        let after_open = trimmed[open_paren_offset + 1..].trim_start_matches([' ', '\t', '\r']);
        let replacement = if after_open.is_empty() {
            format!(r#"reason = "{escaped}""#)
        } else if trimmed.ends_with(',') {
            format!(r#" reason = "{escaped}","#)
        } else {
            format!(r#", reason = "{escaped}""#)
        };
        return Some(Insertion {
            start: close_paren_offset,
            end: close_paren_offset,
            replacement,
        });
    }

    let newline_before_close = head.rfind('\n')?;
    let last_content_line_start = head[..newline_before_close]
        .rfind('\n')
        .map_or(open_paren_offset + 1, |index| index + 1);
    let last_content_line = &head[last_content_line_start..newline_before_close];
    let indent: String = last_content_line
        .chars()
        .take_while(|character| matches!(character, ' ' | '\t'))
        .collect();
    let last_content_trimmed = last_content_line.trim_end_matches([' ', '\t', '\r']);

    if last_content_trimmed.ends_with(',') || last_content_trimmed.is_empty() {
        let insertion = format!("{indent}reason = \"{escaped}\",\n");
        Some(Insertion {
            start: newline_before_close + 1,
            end: newline_before_close + 1,
            replacement: insertion,
        })
    } else {
        let trimmed_end = last_content_line_start + last_content_trimmed.len();
        let replacement = format!(",\n{indent}reason = \"{escaped}\",");
        Some(Insertion {
            start: trimmed_end,
            end: newline_before_close,
            replacement,
        })
    }
}

/// Locate the byte offsets of the outermost `(` and its matching
/// `)` in `snippet`, using `rustc_lexer::tokenize` so comments and
/// string literals don't trip the scan. Returns `None` if the
/// snippet contains no top-level parenthesised group.
fn locate_outermost_parens(snippet: &str) -> Option<(usize, usize)> {
    let mut open: Option<usize> = None;
    let mut depth: usize = 0;
    let mut offset: usize = 0;
    for token in tokenize(snippet, FrontmatterAllowed::No) {
        let len = token.len as usize;
        match token.kind {
            TokenKind::OpenParen => {
                if open.is_none() {
                    open = Some(offset);
                }
                depth += 1;
            }
            TokenKind::CloseParen => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    return open.map(|open_offset| (open_offset, offset));
                }
            }
            _ => {}
        }
        offset += len;
    }
    None
}

/// Render `input` as the body of a Rust `"..."` string literal.
/// Escapes `\\`, `"`, and the C0 / DEL control characters; other
/// characters (including Unicode) pass through unchanged.
pub(super) fn escape_for_rust_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str(r#"\""#),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            '\0' => out.push_str(r"\0"),
            character if (character as u32) < 0x20 || (character as u32) == 0x7F => {
                // Any other C0 byte / DEL: use the `\u{...}` form
                // — the only escape that works for every control
                // codepoint without a dedicated short form.
                let _ = write!(out, "\\u{{{:x}}}", character as u32);
            }
            character => out.push(character),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{build_reason_insertion, escape_for_rust_string};

    fn run_insertion(input: &str, escaped: &str) -> String {
        let insertion =
            build_reason_insertion(input, escaped).expect("build_reason_insertion returned None");
        let mut output = String::new();
        output.push_str(&input[..insertion.start]);
        output.push_str(&insertion.replacement);
        output.push_str(&input[insertion.end..]);
        output
    }

    #[test]
    fn insertion_single_line_no_trailing_comma() {
        assert_eq!(
            run_insertion("allow(foo)", "why"),
            r#"allow(foo, reason = "why")"#,
        );
    }

    #[test]
    fn insertion_single_line_trailing_comma() {
        assert_eq!(
            run_insertion("allow(foo,)", "why"),
            r#"allow(foo, reason = "why",)"#,
        );
    }

    #[test]
    fn insertion_multi_line_trailing_comma() {
        let input = "allow(\n    foo,\n)";
        let expected = "allow(\n    foo,\n    reason = \"why\",\n)";
        assert_eq!(run_insertion(input, "why"), expected);
    }

    /// Wrapped argument list whose closing `)` rides the last
    /// argument's line. The `)` is not on its own line, so the
    /// inline strategy applies — splice before `)` rather than
    /// emitting a stray unindented `reason` line.
    #[test]
    fn insertion_wrapped_close_paren_on_last_arg_line() {
        assert_eq!(
            run_insertion("allow(\n    foo)", "why"),
            "allow(\n    foo, reason = \"why\")",
        );
    }

    #[test]
    fn insertion_multi_line_no_trailing_comma() {
        let input = "allow(\n    foo\n)";
        let expected = "allow(\n    foo,\n    reason = \"why\",\n)";
        assert_eq!(run_insertion(input, "why"), expected);
    }

    #[test]
    fn insertion_escapes_quotes_and_backslashes() {
        assert_eq!(
            run_insertion("allow(foo)", r#"he said \"yes\""#),
            r#"allow(foo, reason = "he said \"yes\"")"#,
        );
    }

    #[test]
    fn escape_passes_through_unicode() {
        assert_eq!(escape_for_rust_string("café"), "café");
    }

    #[test]
    fn escape_handles_quotes_backslash_and_controls() {
        assert_eq!(escape_for_rust_string("a\\b\"c\nd\te"), r#"a\\b\"c\nd\te"#);
        assert_eq!(escape_for_rust_string("\x01"), r"\u{1}");
        assert_eq!(escape_for_rust_string("\x7f"), r"\u{7f}");
    }
}
