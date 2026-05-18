use std::collections::BTreeSet;

use clippy_utils::diagnostics::{span_lint_and_sugg, span_lint_and_then};
use rustc_ast::{Attribute, LitKind, MetaItem, MetaItemInner, MetaItemKind};
use rustc_errors::Applicability;
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{BytePos, Span, Symbol, sym};

use crate::common::{DefaultState, attr_has_reason, resolved_state};

declare_tool_lint! {
    /// ### What it does
    /// Requires every `#[allow(<lints>)]` and `#[expect(<lints>)]`
    /// attribute to carry an explanatory `reason = "..."` field.
    /// `#[allow]` and `#[expect]` are the two levels that fully
    /// silence a lint's output; the project's record of
    /// suppressions needs to know why each one exists.
    ///
    /// The check is purely local — the attribute itself — and does
    /// not depend on any inherited or ambient lint level.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// Suppressions outlive the conditions that justify them. A
    /// bare `#[allow(clippy::too_many_arguments)]` told the original
    /// author to ignore a complaint; six months later, no one knows
    /// whether the rationale was "matches upstream signature",
    /// "intentional over-engineering", or "we'll fix it in the next
    /// refactor". The `reason` field records intent at the moment of
    /// suppression, and rustc renders it back in `unfulfilled_lint_expectations`
    /// notes when a stale `#[expect]` is encountered.
    ///
    /// ### Example
    /// ```rust,ignore
    /// #[allow(clippy::too_many_arguments)]
    /// fn build_fetcher(/* ... */) {}
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// #[allow(clippy::too_many_arguments, reason = "matches pnpm's signature")]
    /// fn build_fetcher(/* ... */) {}
    /// ```
    pub perfectionist::LINT_SILENCE_REASON,
    Warn,
    r#"`#[allow]` / `#[expect]` attribute lacks an explanatory `reason = "..."` field"#,
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::lint_silence_reason";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Lints excluded from the requirement. Useful for project-wide
    /// suppressions whose rationale lives in the project README
    /// rather than per-site. Each entry is the lint's full name as
    /// it appears inside the attribute (`clippy::module_name_repetitions`,
    /// `dead_code`, …).
    exempt_lints: Vec<String>,
    /// Minimum length of the `reason` value. A one- or two-character
    /// reason (`"x"`, `"ok"`) satisfies the literal presence
    /// requirement but conveys nothing; the default floor of 3
    /// excludes those cases. Projects that want a higher bar (e.g.
    /// require a full sentence) can raise it. Set to `0` to disable
    /// the length branch entirely (presence is still enforced).
    min_reason_length: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            exempt_lints: Vec::new(),
            min_reason_length: 3,
        }
    }
}

pub struct LintSilenceReason {
    exempt_lints: BTreeSet<String>,
    min_reason_length: usize,
}

impl LintSilenceReason {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            exempt_lints: config.exempt_lints.into_iter().collect(),
            min_reason_length: config.min_reason_length,
        }
    }
}

impl_lint_pass!(LintSilenceReason => [LINT_SILENCE_REASON]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[LINT_SILENCE_REASON]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("lint_silence_reason", DefaultState::Active) {
        return;
    }
    lint_store.register_early_pass(|| Box::new(LintSilenceReason::new()));
}

impl EarlyLintPass for LintSilenceReason {
    /// `EarlyLintPass::check_attribute` runs once per syntactic
    /// attribute in source — `cfg_attr` is not unwrapped before the
    /// callback. The walker below therefore reaches every silencing
    /// attribute exactly once: bare `#[allow]` / `#[expect]` through
    /// the first branch, `cfg_attr`-wrapped through the second.
    fn check_attribute(&mut self, lint_context: &EarlyContext<'_>, attribute: &Attribute) {
        if is_silencing_attribute_name(attribute.name()) {
            if let Some(args) = attribute.meta_item_list() {
                self.check_silencing(lint_context, attribute.span, &args);
            }
        } else if attribute.has_name(sym::cfg_attr) {
            let Some(cfg_attr_args) = attribute.meta_item_list() else {
                return;
            };
            for wrapped in cfg_attr_args.iter().skip(1) {
                let Some(meta_item) = wrapped.meta_item() else {
                    continue;
                };
                self.walk_cfg_attr_inner(lint_context, meta_item);
            }
        }
    }
}

const SILENCING_NAMES: [Symbol; 2] = [sym::allow, sym::expect];

fn is_silencing_attribute_name(name: Option<Symbol>) -> bool {
    name.is_some_and(|name| SILENCING_NAMES.contains(&name))
}

fn is_silencing_meta_item(meta_item: &MetaItem) -> bool {
    SILENCING_NAMES.iter().any(|name| meta_item.has_name(*name))
}

impl LintSilenceReason {
    /// Recursively walk a meta-item that appears inside a
    /// `cfg_attr` argument list. `cfg_attr` accepts another
    /// `cfg_attr` as its payload — `#[cfg_attr(<cfg_a>,
    /// cfg_attr(<cfg_b>, allow(<lints>)))]` is valid Rust and
    /// the inner `allow` still needs the same check.
    fn walk_cfg_attr_inner(&self, lint_context: &EarlyContext<'_>, meta_item: &MetaItem) {
        if is_silencing_meta_item(meta_item) {
            let MetaItemKind::List(args) = &meta_item.kind else {
                return;
            };
            self.check_silencing(lint_context, meta_item.span, args);
            return;
        }
        if meta_item.has_name(sym::cfg_attr) {
            let MetaItemKind::List(cfg_attr_args) = &meta_item.kind else {
                return;
            };
            for wrapped in cfg_attr_args.iter().skip(1) {
                let Some(inner) = wrapped.meta_item() else {
                    continue;
                };
                self.walk_cfg_attr_inner(lint_context, inner);
            }
        }
    }
}

impl LintSilenceReason {
    /// Apply the check to a single `allow(...)` / `expect(...)` invocation.
    ///
    /// `invocation_span` is the span of the meta-item itself
    /// (covering `allow(...)` / `expect(...)`), used as the autofix
    /// anchor. For a bare `#[allow(...)]` this is the same as
    /// `attribute.meta().span`; for a `cfg_attr`-wrapped one it is
    /// the inner attribute's span, so the suggestion edits the
    /// inner argument list rather than the outer `cfg_attr` wrapper.
    fn check_silencing(
        &self,
        lint_context: &EarlyContext<'_>,
        invocation_span: Span,
        args: &[MetaItemInner],
    ) {
        if self.every_named_lint_is_exempt(args) {
            return;
        }
        match attr_has_reason(args) {
            None => self.emit_missing_field(lint_context, invocation_span),
            Some(literal) => {
                let LitKind::Str(value, _) = literal.kind else {
                    return;
                };
                let char_count = value.as_str().chars().count();
                // An empty literal — `reason = ""` — carries no
                // rationale; the autofix even emits one as its
                // placeholder. Treat it as if the field were
                // missing so the diagnostic message matches and
                // the author is pointed at the literal that needs
                // filling in.
                if char_count == 0 {
                    self.emit_empty_reason(lint_context, literal.span);
                    return;
                }
                if self.min_reason_length == 0 {
                    return;
                }
                if char_count < self.min_reason_length {
                    self.emit_too_short(lint_context, literal.span);
                }
            }
        }
    }

    fn every_named_lint_is_exempt(&self, args: &[MetaItemInner]) -> bool {
        for arg in args {
            let MetaItemInner::MetaItem(meta) = arg else {
                continue;
            };
            // `reason = "..."` is not a lint name; ignore it.
            if matches!(meta.kind, MetaItemKind::NameValue(_)) {
                continue;
            }
            let name = render_lint_name(meta);
            if !self.exempt_lints.contains(&name) {
                return false;
            }
        }
        // Vacuously true for an attribute with zero lint names —
        // `#[allow()]` and the degenerate `#[allow(reason = "...")]`
        // both fall here. The attribute is syntactically valid but
        // silences nothing, so the rule has nothing to flag.
        true
    }

    fn emit_missing_field(&self, lint_context: &EarlyContext<'_>, invocation_span: Span) {
        let insertion = lint_context
            .sess()
            .source_map()
            .span_to_snippet(invocation_span)
            .ok()
            .and_then(|snippet| build_insertion(&snippet));
        let Some(Insertion {
            start,
            end,
            replacement,
        }) = insertion
        else {
            emit_missing_field_no_sugg(lint_context, invocation_span);
            return;
        };
        let lo = invocation_span.lo() + BytePos(start as u32);
        let hi = invocation_span.lo() + BytePos(end as u32);
        let suggestion_span = invocation_span.with_lo(lo).with_hi(hi);
        span_lint_and_sugg(
            lint_context,
            LINT_SILENCE_REASON,
            suggestion_span,
            r#"lint-silencing attribute lacks an explanatory `reason = "..."` field"#,
            "add a `reason` field",
            replacement,
            Applicability::HasPlaceholders,
        );
    }

    fn emit_empty_reason(&self, lint_context: &EarlyContext<'_>, literal_span: Span) {
        span_lint_and_then(
            lint_context,
            LINT_SILENCE_REASON,
            literal_span,
            r#"lint-silencing attribute lacks an explanatory `reason = "..."` field"#,
            |diagnostic| {
                diagnostic.help("write a rationale into the empty `reason` literal");
            },
        );
    }

    fn emit_too_short(&self, lint_context: &EarlyContext<'_>, literal_span: Span) {
        span_lint_and_then(
            lint_context,
            LINT_SILENCE_REASON,
            literal_span,
            "`reason` is shorter than the configured minimum",
            |diagnostic| {
                diagnostic.help(format!(
                    "write a rationale of at least {} character{}",
                    self.min_reason_length,
                    if self.min_reason_length == 1 { "" } else { "s" },
                ));
            },
        );
    }
}

/// Emit the "missing reason" diagnostic without a code suggestion.
/// Used when the source snippet is unavailable (macro expansion) or
/// when [`build_insertion`] cannot make sense of the snippet.
fn emit_missing_field_no_sugg(lint_context: &EarlyContext<'_>, invocation_span: Span) {
    span_lint_and_then(
        lint_context,
        LINT_SILENCE_REASON,
        invocation_span,
        r#"lint-silencing attribute lacks an explanatory `reason = "..."` field"#,
        |diagnostic| {
            diagnostic.help(r#"add a `reason = "..."` argument inside the attribute"#);
        },
    );
}

fn render_lint_name(meta: &MetaItem) -> String {
    let mut result = String::new();
    for (index, segment) in meta.path.segments.iter().enumerate() {
        if index > 0 {
            result.push_str("::");
        }
        result.push_str(segment.ident.name.as_str());
    }
    result
}

/// The text edit produced for the missing-`reason` autofix, expressed
/// as byte offsets inside the meta-item's source snippet.
struct Insertion {
    start: usize,
    end: usize,
    replacement: String,
}

/// Locate the byte offsets of the outermost `(` and its matching
/// `)` in `snippet`, using `rustc_lexer::tokenize` so comments,
/// string literals, and char literals don't trip the scan. Returns
/// `None` if the snippet contains no top-level parenthesised group.
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

/// Compute the byte-offset edit that inserts `reason = ""` into the
/// attribute's argument list. `snippet` is the source text covering
/// at least `allow(...)` / `expect(...)`; offsets are relative to
/// its start. The bare-attribute caller passes `attribute.span`
/// (which includes the `#[` / `#![` wrapper) and the cfg_attr
/// caller passes the inner `meta_item.span` (no wrapper); the
/// scanner is anchored at the outermost `(` / `)` of the snippet
/// regardless of the wrapper.
///
/// The three layouts the planning file enumerates each map to a
/// different splice:
///
/// - Single line, no trailing comma — insert `, reason = ""` right
///   before the closing `)`.
/// - Single line, trailing comma — insert ` reason = "",` right
///   before the closing `)`.
/// - Multi-line — insert a new line `reason = "",` (matching the
///   indentation of the preceding argument) immediately before the
///   line carrying the closing `)`. If the last argument lacks a
///   trailing comma, the edit also adds one at the end of that
///   line.
///
/// Returns `None` if the snippet does not contain the expected
/// `(...)` layout (e.g. macro-expanded sources where
/// `span_to_snippet` returns a synthetic placeholder).
fn build_insertion(snippet: &str) -> Option<Insertion> {
    // Use rustc's own tokenizer to locate the outermost `(` and
    // matching `)` rather than scanning bytes. A naive `rfind(')')`
    // would land inside any block / line comment containing `)` and
    // produce a splice that breaks the attribute. The tokenizer
    // handles comments, string literals, raw strings, and char
    // literals the same way rustc itself does.
    let (open_paren_offset, close_paren_offset) = locate_outermost_parens(snippet)?;
    let head = &snippet[..close_paren_offset];

    if !head[open_paren_offset..].contains('\n') {
        // Single-line layout: trim trailing ASCII whitespace from
        // `head` to see whether the last non-space char is a comma.
        let trimmed = head.trim_end_matches([' ', '\t', '\r']);
        let has_trailing_comma = trimmed.ends_with(',');
        let replacement = if has_trailing_comma {
            r#" reason = "","#
        } else {
            r#", reason = """#
        };
        return Some(Insertion {
            start: close_paren_offset,
            end: close_paren_offset,
            replacement: replacement.to_owned(),
        });
    }

    // Multi-line layout. Locate the line containing `)` and the
    // previous content line whose indent we want to match.
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
        // Either the previous argument already carries a trailing
        // comma, or the `(` line was empty — in both cases we can
        // append our new line just before the `)` line without
        // touching the preceding content.
        let insertion = format!("{indent}reason = \"\",\n");
        Some(Insertion {
            start: newline_before_close + 1,
            end: newline_before_close + 1,
            replacement: insertion,
        })
    } else {
        // Previous argument lacks a trailing comma. Splice over the
        // tail of the last content line so we can both append `,`
        // and insert the new line.
        let trimmed_end = last_content_line_start + last_content_trimmed.len();
        let replacement = format!(",\n{indent}reason = \"\",");
        Some(Insertion {
            start: trimmed_end,
            end: newline_before_close,
            replacement,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Insertion, build_insertion};

    fn run(input: &str) -> String {
        let Insertion {
            start,
            end,
            replacement,
        } = build_insertion(input).expect("build_insertion returned None");
        let mut output = String::new();
        output.push_str(&input[..start]);
        output.push_str(&replacement);
        output.push_str(&input[end..]);
        output
    }

    #[test]
    fn single_line_no_trailing_comma() {
        assert_eq!(run("allow(foo)"), r#"allow(foo, reason = "")"#);
    }

    #[test]
    fn single_line_trailing_comma() {
        assert_eq!(run("allow(foo,)"), r#"allow(foo, reason = "",)"#);
    }

    #[test]
    fn single_line_multiple_lints() {
        assert_eq!(run("allow(foo, bar)"), r#"allow(foo, bar, reason = "")"#);
    }

    #[test]
    fn multi_line_trailing_comma() {
        let input = "allow(\n    foo,\n)";
        let expected = "allow(\n    foo,\n    reason = \"\",\n)";
        assert_eq!(run(input), expected);
    }

    #[test]
    fn multi_line_no_trailing_comma() {
        let input = "allow(\n    foo\n)";
        let expected = "allow(\n    foo,\n    reason = \"\",\n)";
        assert_eq!(run(input), expected);
    }

    #[test]
    fn multi_line_tab_indent() {
        let input = "expect(\n\tfoo,\n)";
        let expected = "expect(\n\tfoo,\n\treason = \"\",\n)";
        assert_eq!(run(input), expected);
    }

    /// `rfind(')')` would land inside the block comment; the
    /// tokenizer-backed scan skips comment contents and finds the
    /// real outer `)`.
    #[test]
    fn snippet_with_block_comment_containing_paren() {
        let input = "#[allow(foo) /* ) */]";
        let expected = r#"#[allow(foo, reason = "") /* ) */]"#;
        assert_eq!(run(input), expected);
    }

    /// Same but with a line comment after the close-paren.
    #[test]
    fn snippet_with_line_comment_after_args() {
        let input = "#[allow(foo) // trailing comment\n]";
        let expected = concat!(r#"#[allow(foo, reason = "") // trailing comment"#, "\n]");
        assert_eq!(run(input), expected);
    }

    /// Block comment *between* arguments.
    #[test]
    fn snippet_with_block_comment_between_args() {
        let input = "#[allow(/* ) */ foo)]";
        let expected = r#"#[allow(/* ) */ foo, reason = "")]"#;
        assert_eq!(run(input), expected);
    }

    /// CRLF line endings should not confuse the trailing-comma
    /// detection — `\r` is treated the same as horizontal
    /// whitespace when scanning the end of the last content line.
    /// The inserted line uses LF; the existing source's CRLF runs
    /// are preserved.
    #[test]
    fn multi_line_crlf_trailing_comma() {
        let input = "allow(\r\n    foo,\r\n)";
        let expected = "allow(\r\n    foo,\r\n    reason = \"\",\n)";
        assert_eq!(run(input), expected);
    }

    /// Empty snippets and snippets with no parens return `None`
    /// so the caller falls back to the no-sugg diagnostic.
    #[test]
    fn empty_or_unparenthesised_snippets_yield_none() {
        assert!(build_insertion("").is_none());
        assert!(build_insertion("allow").is_none());
        assert!(build_insertion("allow(foo").is_none());
    }
}
