//! `perfectionist::prefer_raw_string` — flag cooked string literals
//! whose only escapes are ones a raw string would express verbatim.
//!
//! The hand-rolled parser-combinator scanner lives in [`parser`]; this
//! file owns the lint declaration, the configuration, and the late
//! pass that drives it.

use clippy_utils::diagnostics::span_lint_and_sugg;
use rustc_ast::{LitKind, StrStyle};
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

mod parser;

use parser::{
    DEFAULT_ESCAPES_ELIGIBLE, is_supported_eligible_entry, minimal_hash_count, scan_body,
};

use crate::common::{DefaultState, resolved_state};

declare_tool_lint! {
    /// ### What it does
    /// Forbids regular string literals whose only backslash escapes
    /// are ones a raw string would express verbatim — `\"`, `\\`,
    /// and `\'`. The autofix rewrites the literal to the raw form
    /// `r"..."` / `r#"..."#`, picking the smallest hash count that
    /// avoids a delimiter collision.
    ///
    /// This includes literals passed as arguments to macros such as
    /// `println!`, `format!`, `vec!`, and `assert!`. Suppress per
    /// call site with `#[allow(perfectionist::prefer_raw_string)]`
    /// when the regular form is deliberately preferred.
    ///
    /// Pattern-position literals (e.g. `match s { "C:\\path" => ... }`)
    /// are out of scope — the rule only visits expression literals.
    ///
    /// Whitespace and control-character escapes (`\n`, `\t`, `\r`,
    /// `\0`) and Unicode escapes (`\x..`, `\u{..}`) are exempt — a
    /// raw string cannot express them, and the regular form is the
    /// only choice. A literal that mixes eliminable and
    /// inexpressible escapes is also left alone; the rewrite would
    /// force the author to split the literal or fall back to
    /// `concat!`, which loses more than it gains.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. The
    /// rule trades one noise source (interior backslash escapes)
    /// for a slightly more elaborate string syntax. The benefit is
    /// highest in strings full of file paths, regex patterns, JSON
    /// snippets, or embedded source code — all of which would
    /// otherwise be a sea of `\\` and `\"`.
    ///
    /// ### Example
    /// ```rust,ignore
    /// let json = "{\"name\":\"foo\"}";
    /// let path = "C:\\Users\\foo\\bar";
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// let json = r#"{"name":"foo"}"#;
    /// let path = r"C:\Users\foo\bar";
    /// ```
    pub perfectionist::PREFER_RAW_STRING,
    Warn,
    "string literal contains only raw-expressible escapes; prefer the raw-string form",
    // Load-bearing: an escaped string literal passed as a `println!` /
    // `format!` / `vec!` / etc. argument lives inside a `core` macro
    // expansion. With the default `false` rustc would treat every
    // diagnostic on those literals as "in an external macro" and
    // drop it before reaching the user, even though the literal
    // itself is user-written. The `span_to_snippet` guard in
    // `check_expr` already bails on synthesised spans, so
    // compiler-generated literals stay safely out of scope.
    report_in_external_macro: true
}

const CONFIG_KEY: &str = "perfectionist::prefer_raw_string";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Minimum number of eliminable escapes a string must contain
    /// before the lint fires. Default 1 catches every escapable
    /// string; set to 2 to skip single-escape literals where the
    /// raw form is arguably noisier than the original.
    min_escapes_to_trigger: usize,
    /// Escape sequences considered eliminable by switching to raw
    /// form. Only the three Rust escapes whose decoded character
    /// is exactly the byte after the backslash — `"\""`, `"\\"`,
    /// `"\\'"` — are accepted; entries listed here that fall
    /// outside that closed set are silently dropped. (`\n`, `\t`,
    /// `\xNN`, `\u{...}` and other escapes decode to a different
    /// character and cannot be expressed verbatim in a raw string,
    /// so they have no place in this list.) Use this knob to
    /// narrow eligibility — e.g. `["\\\""]` to only flag literals
    /// whose sole escapes are escaped quotes — not to extend it.
    escapes_eligible: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_escapes_to_trigger: 1,
            escapes_eligible: DEFAULT_ESCAPES_ELIGIBLE
                .iter()
                .map(|entry| (*entry).to_owned())
                .collect(),
        }
    }
}

pub struct PreferRawString {
    min_escapes_to_trigger: usize,
    escapes_eligible: Vec<String>,
}

impl PreferRawString {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        // Drop entries that aren't one of the three self-decoding
        // escapes (`\"`, `\\`, `\'`). Anything else — `\n`, `\t`,
        // `\xNN`, `\u{...}`, ill-formed shapes — would break
        // the parser's "second char is the decoded form" contract
        // and let the `MachineApplicable` autofix silently corrupt
        // user code. Filter rather than reject so a stray entry in
        // the config table doesn't take the whole rule offline.
        let escapes_eligible = config
            .escapes_eligible
            .into_iter()
            .filter(|entry| is_supported_eligible_entry(entry))
            .collect();
        Self {
            min_escapes_to_trigger: config.min_escapes_to_trigger,
            escapes_eligible,
        }
    }
}

impl_lint_pass!(PreferRawString => [PREFER_RAW_STRING]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[PREFER_RAW_STRING]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Disabled = resolved_state("prefer_raw_string", DefaultState::Enabled) {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(PreferRawString::new()));
}

impl<'tcx> LateLintPass<'tcx> for PreferRawString {
    fn check_expr(&mut self, lint_context: &LateContext<'tcx>, expr: &Expr<'tcx>) {
        let ExprKind::Lit(literal) = expr.kind else {
            return;
        };
        if !matches!(literal.node, LitKind::Str(_, StrStyle::Cooked)) {
            return;
        }
        let Ok(snippet) = lint_context
            .sess()
            .source_map()
            .span_to_snippet(literal.span)
        else {
            return;
        };
        // Belt-and-braces: defend against any source spelling that
        // doesn't actually look like a cooked string literal at the
        // syntactic level (synthesised spans, edge cases). The
        // `Cooked` check above already covers the normal path.
        let Some(body) = snippet
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
        else {
            return;
        };
        let Some(scan) = scan_body(body, &self.escapes_eligible) else {
            return;
        };
        // The `eliminable_count == 0` guard is stricter than the
        // planning file's `count >= threshold` rule and deliberate:
        // suggesting `r"hello"` for `"hello"` would just trip
        // `clippy::needless_raw_strings` on the next pass. The
        // guard kicks in if a user sets `min_escapes_to_trigger`
        // to 0, which the planning file doesn't expect but the
        // schema doesn't forbid.
        if scan.eliminable_count == 0 || scan.eliminable_count < self.min_escapes_to_trigger {
            return;
        }
        let n_hashes = minimal_hash_count(&scan.decoded);
        let hashes = "#".repeat(n_hashes);
        let suggestion = format!("r{hashes}\"{}\"{hashes}", scan.decoded);
        span_lint_and_sugg(
            lint_context,
            PREFER_RAW_STRING,
            literal.span,
            "string literal uses escapes that a raw string would avoid",
            "use a raw string",
            suggestion,
            Applicability::MachineApplicable,
        );
    }
}
