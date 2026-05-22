use std::collections::BTreeSet;
use std::fmt::Write as _;

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_ast::{Attribute, MetaItemInner};
use rustc_errors::Applicability;
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{BytePos, Pos, RelativeBytePos, SourceFile, Span, Symbol, SyntaxContext, sym};

use crate::common::{DefaultState, attr_has_reason, resolved_state};

declare_tool_lint! {
    /// ### What it does
    /// When a lint-level attribute (`#[allow]`, `#[expect]`, `#[warn]`,
    /// `#[deny]`, `#[forbid]`) carries an adjacent line comment that
    /// documents *why* the level was chosen, lifts the comment into
    /// the attribute's `reason = "..."` field and removes the
    /// original comment.
    ///
    /// Two placements count:
    ///
    /// - **Trailing.** A `// ...` comment on the same source line as
    ///   the attribute's closing `]`. Highest confidence.
    /// - **Leading.** A `// ...` comment on the previous source line
    ///   (no blank line between, no other attribute between). Lower
    ///   confidence — the comment may also be documentation for the
    ///   next item.
    ///
    /// Doc comments (`///`, `//!`) and block comments (`/* ... */`)
    /// are out of scope.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// `reason = "..."` is part of the attribute and travels with it
    /// through every refactor; a free-floating comment can be
    /// separated from its attribute by an unrelated edit. Compiler
    /// diagnostics render the `reason` field in the lint's message,
    /// so the rationale reaches the reader at the moment of confusion.
    /// One canonical location for the rationale also removes the
    /// "is this comment for the attribute, or for the next item?"
    /// question.
    ///
    /// ### Example
    /// ```rust,ignore
    /// #[allow(clippy::too_many_arguments)] // matches upstream signature
    /// fn build_fetcher(/* ... */) {}
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// #[allow(clippy::too_many_arguments, reason = "matches upstream signature")]
    /// fn build_fetcher(/* ... */) {}
    /// ```
    pub perfectionist::LINT_REASON_FROM_COMMENT,
    Warn,
    r#"adjacent comment on a lint-level attribute should be lifted into a `reason = "..."` field"#,
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::lint_reason_from_comment";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Comment placements considered candidates. Subset of
    /// `["trailing", "leading"]`. The trailing placement is the
    /// canonical one and is the highest-confidence case; the leading
    /// placement is lower confidence because the comment may also be
    /// documentation for the next item.
    sites: Vec<Site>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Site {
    /// `// ...` comment on the same source line as the attribute's
    /// closing `]`.
    Trailing,
    /// `// ...` comment on the previous source line, with no blank
    /// line between the comment and the attribute.
    Leading,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sites: vec![Site::Trailing, Site::Leading],
        }
    }
}

pub struct LintReasonFromComment {
    sites: BTreeSet<Site>,
    /// Source span of the most recently-visited
    /// `sym::cfg_attr_trace` attribute. rustc replaces a
    /// successfully-applied `#[cfg_attr(<cond>, <inner>)]` with a
    /// trace attribute (whose span covers the original
    /// `#[cfg_attr(...)]`) followed by the synthesised `<inner>`
    /// attributes; the synthesised inner attributes carry source
    /// spans pointing at the inner positions *within* the
    /// cfg_attr source, which is too narrow to scan for adjacent
    /// comments. Stashing the trace's outer span here lets the
    /// next synth lint-level attribute use it for the
    /// comment-search anchor. Cleared when we visit an attribute
    /// whose span lies outside the trace's range — the trace's
    /// influence ends as soon as the AST walks past it.
    pending_cfg_attr_outer: Option<Span>,
}

impl LintReasonFromComment {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            sites: config.sites.into_iter().collect(),
            pending_cfg_attr_outer: None,
        }
    }
}

impl_lint_pass!(LintReasonFromComment => [LINT_REASON_FROM_COMMENT]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[LINT_REASON_FROM_COMMENT]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("lint_reason_from_comment", DefaultState::Active)
    {
        return;
    }
    lint_store.register_early_pass(|| Box::new(LintReasonFromComment::new()));
}

const LINT_LEVEL_NAMES: [Symbol; 5] = [sym::allow, sym::expect, sym::warn, sym::deny, sym::forbid];

fn is_lint_level_attribute_name(name: Option<Symbol>) -> bool {
    name.is_some_and(|name| LINT_LEVEL_NAMES.contains(&name))
}

impl EarlyLintPass for LintReasonFromComment {
    /// `EarlyLintPass::check_attribute` visits each syntactic
    /// attribute once. A bare `#[allow(...)]` arrives directly; a
    /// `cfg_attr`-wrapped `#[cfg_attr(<cond>, allow(...))]` is split
    /// by rustc into a `sym::cfg_attr_trace` attribute (carrying the
    /// outer span) and one or more synth `#[allow(...)]` attributes
    /// — see the cfg_attr-trace handling below.
    fn check_attribute(&mut self, lint_context: &EarlyContext<'_>, attribute: &Attribute) {
        if self.sites.is_empty() {
            return;
        }
        // rustc replaces a `#[cfg_attr(<cond>, <inner>)]` whose
        // condition evaluates true with a *trace* attribute named
        // `sym::cfg_attr_trace` (whose span still covers the original
        // `#[cfg_attr(...)]`) followed by the synthesised `<inner>`
        // attribute(s). The trace's `args` are already parsed into a
        // private `AttrItemKind::Parsed(CfgAttrTrace)` payload that
        // is opaque to the public `Attribute` API — there's no way
        // to walk its inner meta items directly. The trace's outer
        // *span* is still available, though, so we stash it as we
        // visit the trace and use it as the comment-search anchor
        // for the synth lint-level attributes that follow.
        if attribute.has_name(sym::cfg_attr_trace) {
            // Nested cfg_attr: a `#[cfg_attr(<a>, cfg_attr(<b>, allow(...)))]`
            // produces an outer trace, then an inner trace, then the synth
            // `allow(...)`. The inner trace's span covers only the inner
            // `cfg_attr(<b>, allow(...))` and would miss a trailing comment
            // on the outermost `]`. Preserve the outermost trace when its
            // span already encloses the new one.
            match self.pending_cfg_attr_outer {
                Some(existing) if existing.contains(attribute.span) => {}
                _ => self.pending_cfg_attr_outer = Some(attribute.span),
            }
            return;
        }
        let outer_span = match self.pending_cfg_attr_outer {
            Some(trace_span) if trace_span.contains(attribute.span) => trace_span,
            _ => {
                self.pending_cfg_attr_outer = None;
                attribute.span
            }
        };
        if is_lint_level_attribute_name(attribute.name())
            && let Some(args) = attribute.meta_item_list()
        {
            let emitted = self.check(lint_context, outer_span, attribute.span, &args);
            // A cfg_attr trace is one comment anchor for *one* adjacent
            // source comment; if the cfg_attr expands into multiple
            // lint-level synth attributes (e.g.
            // `#[cfg_attr(all(), allow(a), warn(b))]`), only the first
            // one to actually lift the comment should consume the
            // trace — otherwise every synth attr would emit a duplicate
            // suggestion whose `delete_span` overlaps the others on the
            // same comment bytes, and rustfix would refuse to apply
            // the conflicting edits. Conversely, if `check` declined
            // to emit (e.g. the first synth already carries a `reason`
            // field, or no adjacent comment was found), the trace must
            // stay live so the next synth in the same cfg_attr can
            // still find it.
            if emitted && self.pending_cfg_attr_outer == Some(outer_span) {
                self.pending_cfg_attr_outer = None;
            }
        }
    }
}

impl LintReasonFromComment {
    /// Apply the trigger and emission for one lint-level invocation.
    ///
    /// `outer_span` is the syntactic attribute's span (the one to
    /// scan for adjacent comments). `invocation_span` is the inner
    /// `allow(...)` / `expect(...)` / `warn(...)` / `deny(...)` /
    /// `forbid(...)` meta-item's span — the insertion target for the
    /// lifted `reason` field. For a bare `#[allow(...)]` the two are
    /// the same.
    /// Returns `true` iff the rule actually emitted a diagnostic
    /// (used by [`check_attribute`] to decide whether to consume
    /// the pending `cfg_attr_trace` outer span).
    fn check(
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
        let Some(outer_lo) = outer_span.lo().0.checked_sub(file_start.0) else {
            return false;
        };
        let Some(outer_hi) = outer_span.hi().0.checked_sub(file_start.0) else {
            return false;
        };
        let outer_lo = outer_lo as usize;
        let outer_hi = outer_hi as usize;
        if outer_hi > source_text.len() {
            return false;
        }

        // Try the higher-confidence trailing placement first. The
        // planning file makes the two placements mutually exclusive —
        // a comment immediately before the attribute and a comment
        // on the same line as the closing `]` are unrelated source
        // edits — so the first match wins. `find_*_comment` filter
        // out empty-normalised matches, so an unhelpful trailing `//`
        // does not pre-empt a real leading comment.
        if self.sites.contains(&Site::Trailing)
            && let Some(comment) = find_trailing_comment(source_text, outer_hi)
        {
            self.emit(
                lint_context,
                &source_file,
                invocation_span,
                &comment,
                Applicability::MachineApplicable,
            );
            return true;
        }
        if self.sites.contains(&Site::Leading)
            && let Some(comment) = find_leading_comment(source_text, outer_lo)
        {
            self.emit(
                lint_context,
                &source_file,
                invocation_span,
                &comment,
                Applicability::MaybeIncorrect,
            );
            return true;
        }
        false
    }

    fn emit(
        &self,
        lint_context: &EarlyContext<'_>,
        source_file: &SourceFile,
        invocation_span: Span,
        comment: &Comment,
        applicability: Applicability,
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

        let diag_span = file_span(source_file, comment.diag_start, comment.diag_end);
        let delete_span = file_span(source_file, comment.delete_start, comment.delete_end);

        span_lint_and_then(
            lint_context,
            LINT_REASON_FROM_COMMENT,
            diag_span,
            "comment can be lifted into the attribute's `reason` field",
            |diagnostic| {
                diagnostic.multipart_suggestion(
                    "lift the comment into a `reason` field",
                    vec![
                        (arg_span, insertion.replacement),
                        (delete_span, String::new()),
                    ],
                    applicability,
                );
            },
        );
    }
}

/// Build an absolute [`Span`] covering `[start, end)` byte offsets
/// inside `source_file`. The comment positions are file-local, so the
/// span has the root [`SyntaxContext`] — there is no macro expansion
/// to inherit a context from.
fn file_span(source_file: &SourceFile, start: usize, end: usize) -> Span {
    let lo = source_file.absolute_position(RelativeBytePos::from_usize(start));
    let hi = source_file.absolute_position(RelativeBytePos::from_usize(end));
    Span::new(lo, hi, SyntaxContext::root(), None)
}

/// Adjacent comment located by [`find_trailing_comment`] or
/// [`find_leading_comment`]. Byte offsets are relative to the source
/// file's start (i.e. inside `SourceFile::src`).
struct Comment {
    /// Normalised text to put inside the `reason = "..."` literal.
    text: String,
    /// Range of bytes whose removal makes the comment disappear from
    /// source. For a trailing comment this is the run of horizontal
    /// whitespace + the `// ...` text (not the newline that
    /// terminates the line). For a leading comment this is the whole
    /// line including its trailing line terminator, so removing it
    /// leaves no blank line behind.
    delete_start: usize,
    delete_end: usize,
    /// Range of bytes covering the comment text proper (`//` through
    /// to the end of the comment). Used as the diagnostic's primary
    /// span.
    diag_start: usize,
    diag_end: usize,
}

/// Scan forward from the attribute's closing `]` for a trailing
/// comment on the same source line. Returns `None` if there is no
/// `//` comment between `]` and the next newline, if the only
/// content there is a doc-comment marker (`///`, `//!`), or if there
/// is a non-comment token in the way.
fn find_trailing_comment(source: &str, attr_hi: usize) -> Option<Comment> {
    let bytes = source.as_bytes();
    let mut cursor = attr_hi;
    while cursor < bytes.len() && is_horizontal_whitespace(bytes[cursor]) {
        cursor += 1;
    }
    if cursor + 2 > bytes.len() || &bytes[cursor..cursor + 2] != b"//" {
        return None;
    }
    if is_doc_comment_prefix(&bytes[cursor + 2..]) {
        return None;
    }
    let comment_start = cursor;
    let mut end = comment_start;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }
    // A line that ends `\r\n` has a `\r` immediately before the
    // newline; strip it so the lifted text doesn't carry a stray
    // carriage return.
    let text_end = if end > comment_start && bytes[end - 1] == b'\r' {
        end - 1
    } else {
        end
    };
    let text = normalise_comment_text(&source[comment_start..text_end]);
    // A `//` / `//   ` line normalises to empty; lifting it would
    // produce a vacuous `reason = ""`. Treating it as no-match here
    // lets `check` fall through to the leading-comment placement
    // instead of short-circuiting.
    if text.is_empty() {
        return None;
    }
    Some(Comment {
        text,
        delete_start: attr_hi,
        delete_end: end,
        diag_start: comment_start,
        diag_end: text_end,
    })
}

/// Scan backward from the attribute's opening `#` for a `// ...`
/// comment on the immediately preceding source line. Returns `None`
/// unless the previous line consists *only* of a regular line comment
/// (after any leading indentation) and is separated from the
/// attribute by exactly one line terminator.
fn find_leading_comment(source: &str, attr_lo: usize) -> Option<Comment> {
    let bytes = source.as_bytes();
    if attr_lo > bytes.len() {
        return None;
    }
    // Walk back over the attribute's indentation on its own line.
    let mut cursor = attr_lo;
    while cursor > 0 && is_horizontal_whitespace(bytes[cursor - 1]) {
        cursor -= 1;
    }
    // The attribute must start at the beginning of a line — the byte
    // immediately before the run of horizontal whitespace must be a
    // newline. A non-newline (or start-of-file) means the attribute
    // shares a line with earlier content (`#[other_attr] #[allow]`
    // chains, an inner attribute mid-expression), in which case a
    // leading comment is not meaningful.
    if cursor == 0 || bytes[cursor - 1] != b'\n' {
        return None;
    }
    let attr_line_start = cursor;
    // The `\n` is at `attr_line_start - 1`. Step past it and any
    // immediately-preceding `\r` so the deletion range covers the
    // whole line terminator.
    let newline_pos = attr_line_start - 1;
    let prev_line_terminator_start = if newline_pos > 0 && bytes[newline_pos - 1] == b'\r' {
        newline_pos - 1
    } else {
        newline_pos
    };
    // Walk back to the start of the previous line.
    let mut prev_line_start = prev_line_terminator_start;
    while prev_line_start > 0 && bytes[prev_line_start - 1] != b'\n' {
        prev_line_start -= 1;
    }
    // Skip the previous line's own indentation.
    let mut comment_start = prev_line_start;
    while comment_start < prev_line_terminator_start
        && is_horizontal_whitespace(bytes[comment_start])
    {
        comment_start += 1;
    }
    if comment_start + 2 > prev_line_terminator_start
        || &bytes[comment_start..comment_start + 2] != b"//"
    {
        return None;
    }
    if is_doc_comment_prefix(&bytes[comment_start + 2..prev_line_terminator_start]) {
        return None;
    }
    let text = normalise_comment_text(&source[comment_start..prev_line_terminator_start]);
    // Same empty-normalised-text filter as `find_trailing_comment`:
    // a blank `//` line is not a rationale to lift.
    if text.is_empty() {
        return None;
    }
    Some(Comment {
        text,
        delete_start: prev_line_start,
        delete_end: attr_line_start,
        diag_start: comment_start,
        diag_end: prev_line_terminator_start,
    })
}

fn is_horizontal_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t')
}

/// Whether the bytes immediately *after* a `//` open the comment as
/// a doc comment rather than a regular comment.
///
/// rustc treats `//!` as an inner doc comment and a *three-slash*
/// `///` (third slash not followed by another `/`) as an outer doc
/// comment. A `////` run with four or more slashes is a regular
/// comment again — the same classification rustc_lexer uses.
fn is_doc_comment_prefix(rest_after_slashes: &[u8]) -> bool {
    match rest_after_slashes {
        [b'!', ..] => true,
        [b'/'] => true,
        [b'/', next, ..] if *next != b'/' => true,
        _ => false,
    }
}

/// Strip the `//` marker, trim ASCII whitespace, and strip a leading
/// run of decoration characters (`-`, `=`, `*`) followed by
/// whitespace.
///
/// The input `content` is expected to start with `//`. It is not the
/// raw line — trailing `\r` and similar line-terminator bytes are
/// already removed by the caller.
fn normalise_comment_text(content: &str) -> String {
    let after_slashes = content.strip_prefix("//").unwrap_or(content);
    let trimmed = after_slashes.trim_matches([' ', '\t', '\r']);
    let bytes = trimmed.as_bytes();
    let mut run = 0;
    while run < bytes.len() && matches!(bytes[run], b'-' | b'=' | b'*') {
        run += 1;
    }
    if run > 0
        && bytes
            .get(run)
            .is_some_and(|byte| is_horizontal_whitespace(*byte))
    {
        let after = &trimmed[run..];
        return after.trim_start_matches([' ', '\t']).to_owned();
    }
    trimmed.to_owned()
}

/// Render `input` as the body of a Rust `"..."` string literal.
/// Escapes `\\`, `"`, and the C0 / DEL control characters; other
/// characters (including Unicode) pass through unchanged.
fn escape_for_rust_string(input: &str) -> String {
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

/// The text edit produced for the args-list insertion, expressed
/// as byte offsets inside the meta-item's source snippet. Mirrors
/// the `Insertion` shape used by `lint_silence_reason`, but the
/// `reason` literal carries the lifted comment's text rather than
/// an empty placeholder.
struct Insertion {
    start: usize,
    end: usize,
    replacement: String,
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

/// Compute the byte-offset edit that inserts
/// `reason = "<escaped>"` into the attribute's argument list. The
/// three layout cases match the planning file (and the sibling
/// `lint_silence_reason`'s autofix):
///
/// - **Single line, no trailing comma**: insert
///   `, reason = "<escaped>"` before the closing `)`.
/// - **Single line, trailing comma**: insert
///   ` reason = "<escaped>",` before the closing `)`.
/// - **Multi-line**: insert a new line `reason = "<escaped>",`
///   immediately before the line carrying the closing `)`, matching
///   the indentation of the preceding argument. If the last argument
///   lacks a trailing comma, one is added when inserting.
///
/// Returns `None` if the snippet does not contain the expected
/// `(...)` layout (e.g. macro-expanded sources where
/// `span_to_snippet` returns a synthetic placeholder).
fn build_reason_insertion(snippet: &str, escaped: &str) -> Option<Insertion> {
    let (open_paren_offset, close_paren_offset) = locate_outermost_parens(snippet)?;
    let head = &snippet[..close_paren_offset];

    if !head[open_paren_offset..].contains('\n') {
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

#[cfg(test)]
mod tests {
    use super::{
        Comment, build_reason_insertion, escape_for_rust_string, find_leading_comment,
        find_trailing_comment, normalise_comment_text,
    };

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

    #[test]
    fn normalise_strips_markers_and_decoration() {
        assert_eq!(normalise_comment_text("// hello"), "hello");
        assert_eq!(normalise_comment_text("//   hello  "), "hello");
        assert_eq!(normalise_comment_text("//-- hello"), "hello");
        assert_eq!(normalise_comment_text("//== hello"), "hello");
        assert_eq!(normalise_comment_text("//* hello"), "hello");
        // Decoration without trailing whitespace passes through.
        assert_eq!(normalise_comment_text("//--hello"), "--hello");
        // A non-recognised decoration prefix passes through.
        assert_eq!(normalise_comment_text("// > quoted"), "> quoted");
    }

    /// A bare `//` or whitespace-only `//   ` line normalises to an
    /// empty string. The `emit` guard turns these into no-ops; the
    /// scanner itself still matches them so the scanner stays
    /// position-agnostic (the planning file's filter set is "`//`
    /// line comments that aren't doc comments", and "non-empty" is
    /// not part of that filter).
    #[test]
    fn normalise_collapses_empty_and_whitespace_only_comments() {
        assert_eq!(normalise_comment_text("//"), "");
        assert_eq!(normalise_comment_text("//   "), "");
        assert_eq!(normalise_comment_text("//\t"), "");
    }

    fn assert_comment(actual: Option<Comment>, expected_text: &str) {
        let comment = actual.expect("expected a comment match");
        assert_eq!(comment.text, expected_text);
    }

    #[test]
    fn trailing_simple() {
        let source = "#[allow(foo)] // hello\n";
        let attr_hi = source.find(']').unwrap() + 1;
        assert_comment(find_trailing_comment(source, attr_hi), "hello");
    }

    #[test]
    fn trailing_skips_doc_marker() {
        let source = "#[allow(foo)] /// hello\n";
        let attr_hi = source.find(']').unwrap() + 1;
        assert!(find_trailing_comment(source, attr_hi).is_none());
    }

    #[test]
    fn trailing_skips_inner_doc_marker() {
        let source = "#[allow(foo)] //! hello\n";
        let attr_hi = source.find(']').unwrap() + 1;
        assert!(find_trailing_comment(source, attr_hi).is_none());
    }

    #[test]
    fn trailing_accepts_quadruple_slash() {
        let source = "#[allow(foo)] //// hello\n";
        let attr_hi = source.find(']').unwrap() + 1;
        // `////` is a regular comment in rustc_lexer's classification;
        // the rule lifts its text minus the `//` marker. Normalisation
        // doesn't strip extra slashes; that's left for the author to
        // tidy up after the autofix.
        assert_comment(find_trailing_comment(source, attr_hi), "// hello");
    }

    #[test]
    fn trailing_no_comment_on_same_line() {
        let source = "#[allow(foo)]\n// next line\n";
        let attr_hi = source.find(']').unwrap() + 1;
        assert!(find_trailing_comment(source, attr_hi).is_none());
    }

    #[test]
    fn trailing_handles_crlf() {
        let source = "#[allow(foo)] // hello\r\n";
        let attr_hi = source.find(']').unwrap() + 1;
        assert_comment(find_trailing_comment(source, attr_hi), "hello");
    }

    #[test]
    fn leading_simple() {
        let source = "// hello\n#[allow(foo)]\n";
        let attr_lo = source.find('#').unwrap();
        assert_comment(find_leading_comment(source, attr_lo), "hello");
    }

    #[test]
    fn leading_with_indentation() {
        let source = "    // hello\n    #[allow(foo)]\n";
        let attr_lo = source.find('#').unwrap();
        assert_comment(find_leading_comment(source, attr_lo), "hello");
    }

    #[test]
    fn leading_skips_doc_marker() {
        let source = "/// hello\n#[allow(foo)]\n";
        let attr_lo = source.find('#').unwrap();
        assert!(find_leading_comment(source, attr_lo).is_none());
    }

    #[test]
    fn leading_rejects_blank_line_between() {
        let source = "// hello\n\n#[allow(foo)]\n";
        let attr_lo = source.find('#').unwrap();
        assert!(find_leading_comment(source, attr_lo).is_none());
    }

    #[test]
    fn leading_rejects_other_attribute_on_prev_line() {
        let source = "#[other]\n#[allow(foo)]\n";
        let attr_lo = source.rfind('#').unwrap();
        assert!(find_leading_comment(source, attr_lo).is_none());
    }

    #[test]
    fn leading_rejects_when_prev_line_has_other_content() {
        let source = "let x = 1; // x\n#[allow(foo)]\n";
        let attr_lo = source.find('#').unwrap();
        assert!(find_leading_comment(source, attr_lo).is_none());
    }
}
