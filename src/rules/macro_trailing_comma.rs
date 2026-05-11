use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_sugg;
use rustc_ast::MacCall;
use rustc_ast::token::TokenKind;
use rustc_ast::tokenstream::TokenTree;
use rustc_errors::Applicability;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

declare_tool_lint! {
    /// ### What it does
    /// For function-like macro invocations whose top-level arguments are
    /// comma-separated, enforces rustfmt's `trailing_comma = "Vertical"`
    /// policy that rustfmt itself does not apply inside macro bodies:
    /// multi-line invocations must end with a trailing comma; single-line
    /// invocations must not.
    ///
    /// Eligibility is name-based — a curated list of `core` / `std` and
    /// well-known third-party macros (`vec!`, `format!`, `println!`,
    /// `assert_eq!`, `dbg!`, `log::info!`, `tracing::debug!`,
    /// `anyhow::bail!`, `maplit::hashmap!`, …), extended via
    /// `extra_name_based` and overridden via `ignore`.
    ///
    /// Attribute-style invocations (`#[derive(...)]`, `#[serde(...)]`,
    /// etc.) are out of scope.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. rustfmt's
    /// default `trailing_comma = "Vertical"` policy keeps argument lists
    /// uniform: every multi-line list ends with a comma, every single-line
    /// list does not. rustfmt opts out of macro bodies because a macro
    /// matcher *can* make the trailing comma load-bearing; for the curated
    /// macros covered by this lint, it cannot, and the policy applies
    /// without risk.
    ///
    /// ### Example
    /// ```rust,ignore
    /// let xs = vec![
    ///     1,
    ///     2,
    ///     3
    /// ];
    /// let ys = vec![1, 2, 3,];
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// let xs = vec![
    ///     1,
    ///     2,
    ///     3,
    /// ];
    /// let ys = vec![1, 2, 3];
    /// ```
    pub perfectionist::MACRO_TRAILING_COMMA,
    Warn,
    "macro invocation does not follow rustfmt's vertical trailing-comma policy",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::macro_trailing_comma";

/// Curated macros whose top-level argument list is comma-separated with
/// a syntactically optional trailing comma. See the rule docs in
/// `planned-rules/macro-trailing-comma.md` for the inclusion criterion.
///
/// Each entry is a single segment; matching is by the final segment of
/// the invocation's path, so `vec!`, `std::vec!`, and `::std::vec!` all
/// match the `"vec"` entry.
const BUILTIN_NAME_BASED: &[&str] = &[
    // `core` / `std`
    "vec",
    "format",
    "format_args",
    "print",
    "println",
    "eprint",
    "eprintln",
    "write",
    "writeln",
    "panic",
    "unimplemented",
    "todo",
    "unreachable",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "matches",
    "dbg",
    "concat",
    "env",
    "option_env",
    // `pretty_assertions` (its `assert_eq` / `assert_ne` final segments
    // already match the `core` entries; `assert_str_eq` is unique to it).
    "assert_str_eq",
    // `maplit`
    "hashmap",
    "btreemap",
    "hashset",
    "btreeset",
    "convert_args",
    // `log` (its `error` / `warn` / `info` / `debug` / `trace` final
    // segments also cover `tracing`'s same-named macros).
    "log",
    "error",
    "warn",
    "info",
    "debug",
    "trace",
    // `tracing`
    "event",
    "span",
    // `anyhow`
    "anyhow",
    "bail",
    "ensure",
];

#[derive(Debug, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {
    /// Master on/off switch for the rule. Defaults to `true`. Set
    /// to `false` to silence every diagnostic this lint would emit
    /// without having to enumerate every macro under `ignore`.
    enabled: bool,
    /// Accepted for forward compatibility with the matcher-based half of
    /// the rule. Currently a no-op — only name-based eligibility is
    /// implemented; see `planned-rules/macro-trailing-comma.md` for the
    /// status breakdown.
    matcher_based: bool,
    /// Additional macro paths to treat as name-based eligible, on top
    /// of the curated built-in list. Each entry is matched by its
    /// final path segment, so `"my_crate::vec_like"` and `"vec_like"`
    /// both target invocations whose last segment is `vec_like`.
    /// Empty by default. Only add macros whose trailing comma is
    /// syntactically optional at the top level; macros that treat
    /// the comma as a fully optional separator throughout (rather
    /// than only at the tail) should not be listed here.
    extra_name_based: Vec<String>,
    /// Macro paths to opt out of the rule, even if they would
    /// otherwise be eligible via the built-in list or
    /// `extra_name_based`. Matched by final path segment, like
    /// `extra_name_based`. Checked first, so this knob always wins
    /// over eligibility. Empty by default.
    ignore: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            matcher_based: true,
            extra_name_based: Vec::new(),
            ignore: Vec::new(),
        }
    }
}

pub struct MacroTrailingComma {
    enabled: bool,
    // TODO(matcher_based): the lookup is currently linear
    // (`entries.iter().any(...)`), so the `BTreeSet` ordering is unused
    // — it only deduplicates identical config entries. When the
    // matcher-based half grows these lists, bucket by entry length: a
    // `BTreeSet<String>` for single-segment entries (O(log N) on the
    // invocation's final segment) plus a `Vec<Vec<String>>` for
    // multi-segment entries.
    name_based: BTreeSet<Vec<String>>,
    ignore: BTreeSet<Vec<String>>,
}

impl MacroTrailingComma {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let mut name_based: BTreeSet<Vec<String>> = BUILTIN_NAME_BASED
            .iter()
            .map(|name| vec![(*name).to_owned()])
            .collect();
        for entry in &config.extra_name_based {
            let parsed = parse_path(entry);
            if !parsed.is_empty() {
                name_based.insert(parsed);
            }
        }
        let ignore = config
            .ignore
            .iter()
            .map(|entry| parse_path(entry))
            .filter(|parsed| !parsed.is_empty())
            .collect();
        Self {
            enabled: config.enabled,
            name_based,
            ignore,
        }
    }
}

impl_lint_pass!(MacroTrailingComma => [MACRO_TRAILING_COMMA]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[MACRO_TRAILING_COMMA]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    // Pre-expansion is required so that the visitor still sees `MacCall`
    // nodes. By the post-expansion early pass, the macros covered by this
    // rule have been expanded away and `check_mac` would never fire.
    // The `pre_expansion_passes` slot is the same one Clippy uses for
    // similar macro-shape checks.
    lint_store.register_pre_expansion_pass(|| Box::new(MacroTrailingComma::new()));
}

impl EarlyLintPass for MacroTrailingComma {
    fn check_mac(&mut self, lint_context: &EarlyContext<'_>, mac_call: &MacCall) {
        if !self.enabled {
            return;
        }
        if matches_any(&mac_call.path, &self.ignore) {
            return;
        }
        if !matches_any(&mac_call.path, &self.name_based) {
            return;
        }
        self.check_invocation(lint_context, mac_call);
    }
}

impl MacroTrailingComma {
    fn check_invocation(&self, lint_context: &EarlyContext<'_>, mac_call: &MacCall) {
        let args = &mac_call.args;
        // Single-pass walk over the top-level token stream: track the
        // last tree and bail on a top-level `;`. Avoids allocating a
        // `Vec` per `check_mac` call.
        let mut last_tree: Option<&TokenTree> = None;
        for tree in args.tokens.iter() {
            if let TokenTree::Token(token, _) = tree
                && token.kind == TokenKind::Semi
            {
                return;
            }
            last_tree = Some(tree);
        }
        let Some(last_tree) = last_tree else {
            return;
        };
        let source_map = lint_context.sess().source_map();
        let is_multi_line = source_map.is_multiline(args.dspan.entire());
        let last_is_comma = matches!(
            last_tree,
            TokenTree::Token(token, _) if token.kind == TokenKind::Comma,
        );
        match (is_multi_line, last_is_comma) {
            (true, false) => emit_insert(lint_context, last_tree.span().shrink_to_hi()),
            (false, true) => emit_remove(lint_context, last_tree.span()),
            _ => {}
        }
    }
}

fn parse_path(raw: &str) -> Vec<String> {
    raw.split("::")
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .collect()
}

fn matches_any(invocation: &rustc_ast::Path, entries: &BTreeSet<Vec<String>>) -> bool {
    entries.iter().any(|entry| entry_matches(entry, invocation))
}

/// Match a configured entry against an invocation path without
/// allocating a `Vec<String>` snapshot of the invocation. Single-
/// segment entries match the path's final segment; multi-segment
/// entries tail-match the path's segments.
fn entry_matches(entry: &[String], invocation: &rustc_ast::Path) -> bool {
    let segments = &invocation.segments;
    if entry.is_empty() || segments.is_empty() {
        return false;
    }
    if entry.len() == 1 {
        // Single-segment entry: match by the final segment of the path,
        // so `vec!`, `std::vec!`, and `::std::vec!` all qualify.
        segments
            .last()
            .is_some_and(|segment| segment.ident.name.as_str() == entry[0])
    } else if segments.len() < entry.len() {
        false
    } else {
        // Multi-segment entry: tail-match against the invocation path,
        // accommodating optional leading crate prefixes.
        let start = segments.len() - entry.len();
        segments[start..]
            .iter()
            .zip(entry.iter())
            .all(|(segment, entry_segment)| segment.ident.name.as_str() == entry_segment.as_str())
    }
}

fn emit_insert(lint_context: &EarlyContext<'_>, insert_at: Span) {
    span_lint_and_sugg(
        lint_context,
        MACRO_TRAILING_COMMA,
        insert_at,
        "multi-line macro invocation should end with a trailing comma",
        "add a trailing comma",
        ",".to_owned(),
        Applicability::MachineApplicable,
    );
}

fn emit_remove(lint_context: &EarlyContext<'_>, comma_span: Span) {
    span_lint_and_sugg(
        lint_context,
        MACRO_TRAILING_COMMA,
        comma_span,
        "single-line macro invocation should not end with a trailing comma",
        "remove the trailing comma",
        String::new(),
        Applicability::MachineApplicable,
    );
}
