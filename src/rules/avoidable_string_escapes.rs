use crate::rule_index::{AvoidableStringEscapesRule, Register};
use clippy_utils::diagnostics::span_lint_and_sugg;
use core::num::NonZeroUsize;
use rustc_ast::{LitKind, StrStyle};
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use std::collections::BTreeSet;
use std::sync::Mutex;

mod early;
mod emit;
mod parser;
mod queue;

use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::find_enclosing_hir_ids;
use early::AvoidableStringEscapesEarly;
use parser::{
    DEFAULT_ELIGIBLE_ESCAPES, build_raw_string_suggestion, is_supported_eligible_entry, scan_body,
};
use queue::PendingViolation;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Forbids regular string literals whose only backslash escapes
    /// are ones a raw string would express verbatim — `\"`, `\\`,
    /// and `\'`. The autofix rewrites the literal to the raw form
    /// `r"..."` / `r#"..."#`, picking the smallest hash count that
    /// avoids a delimiter collision.
    ///
    /// Literals inside macro invocations are covered too: every string
    /// literal in a macro call, whatever its position — including a
    /// `format!`-family template that contains a `{...}` placeholder. The
    /// rewrite is value-preserving (a raw string still parses
    /// placeholders, and `{{` / `}}` survive verbatim), so the literal's
    /// role doesn't matter. Suppress a site where the regular form is
    /// deliberately preferred.
    ///
    /// That includes literals a macro uses for their *source spelling*
    /// rather than their value, such as `stringify!` and `dbg!`, where
    /// the raw form has the same value but a different reflected text.
    /// This is intentional: code whose behaviour depends on a literal's
    /// exact spelling instead of its value is rare and a code smell;
    /// suppress it per site with
    /// `#[expect(perfectionist::avoidable_string_escapes)]`.
    ///
    /// Pattern-position literals in ordinary code
    /// (e.g. `match s { "C:\\path" => ... }`) are out of scope; only
    /// expression-position literals are rewritten. A literal written as a
    /// pattern *inside a macro call* (e.g. `matches!(s, "C:\\path")`) is
    /// rewritten anyway, since a literal's position isn't distinguished
    /// inside a macro.
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
    ///
    /// This is a stylistic preference, not a correctness issue. The
    /// rule trades one noise source (interior backslash escapes)
    /// for a slightly more elaborate string syntax. The benefit is
    /// highest in strings full of file paths, regex patterns, JSON
    /// snippets, or embedded source code — all of which would
    /// otherwise be a sea of `\\` and `\"`.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let json = "{\"name\":\"foo\"}";
    /// let path = "C:\\Users\\foo\\bar";
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let json = r#"{"name":"foo"}"#;
    /// let path = r"C:\Users\foo\bar";
    /// ```
    pub perfectionist::AVOIDABLE_STRING_ESCAPES,
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

const CONFIG_KEY: &str = "perfectionist::avoidable_string_escapes";

/// Diagnostic message, shared by the late inline path and the queued
/// pre-expansion path ([`emit::emit_raw_string`]) so the two firing
/// routes read identically to the user.
pub(super) const VIOLATION_MESSAGE: &str =
    "string literal uses escapes that a raw string would avoid";
/// Suggestion label, shared for the same reason as [`VIOLATION_MESSAGE`].
pub(super) const SUGGESTION_LABEL: &str = "use a raw string";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Minimum number of eliminable escapes a string must contain
    /// before the lint fires. Default `1` catches every escapable
    /// string; set to `2` to skip single-escape literals where the
    /// raw form is arguably noisier than the original. The lower
    /// bound is `1` — `0` is rejected at parse time, since
    /// suggesting `r"hello"` for `"hello"` would just trip
    /// `clippy::needless_raw_strings` on the next pass, and a
    /// minimum of `1` already excludes that case.
    min_escapes_to_trigger: NonZeroUsize,
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
    eligible_escapes: Vec<String>,
}

/// Default floor for `min_escapes_to_trigger`. One eliminable
/// escape is enough to make the raw form an unambiguous win.
const DEFAULT_MIN_ESCAPES_TO_TRIGGER: NonZeroUsize = NonZeroUsize::new(1).expect("1 is non-zero");

impl Default for Config {
    fn default() -> Self {
        Self {
            min_escapes_to_trigger: DEFAULT_MIN_ESCAPES_TO_TRIGGER,
            eligible_escapes: DEFAULT_ELIGIBLE_ESCAPES
                .iter()
                .map(|entry| (*entry).to_owned())
                .collect(),
        }
    }
}

/// The rule's settings after parsing and validation, shared by the late
/// `ExprKind::Lit` pass and the pre-expansion macro-literal pass so both
/// apply the same threshold and eligible-escape set.
pub(super) struct ResolvedConfig {
    pub(super) min_escapes_to_trigger: NonZeroUsize,
    pub(super) eligible_escapes: Vec<String>,
}

/// Load and validate the rule's configuration from the shared
/// [`CONFIG_KEY`] table. Called once per pass — the early and late passes
/// each build their own copy — so the validation lives here instead of
/// being duplicated across them; the result is not cached.
pub(super) fn resolved_config() -> ResolvedConfig {
    let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
    // Drop entries that aren't one of the three self-decoding
    // escapes (`\"`, `\\`, `\'`). Anything else — `\n`, `\t`,
    // `\xNN`, `\u{...}`, ill-formed shapes — would break
    // the parser's "second char is the decoded form" contract
    // and let the `MachineApplicable` autofix silently corrupt
    // user code. Filter rather than reject so a stray entry in
    // the config table doesn't take the whole rule offline.
    let eligible_escapes = config
        .eligible_escapes
        .into_iter()
        .filter(|entry| is_supported_eligible_entry(entry))
        .collect();
    ResolvedConfig {
        min_escapes_to_trigger: config.min_escapes_to_trigger,
        eligible_escapes,
    }
}

pub struct AvoidableStringEscapes {
    min_escapes_to_trigger: NonZeroUsize,
    eligible_escapes: Vec<String>,
}

impl AvoidableStringEscapes {
    fn new() -> Self {
        let resolved = resolved_config();
        Self {
            min_escapes_to_trigger: resolved.min_escapes_to_trigger,
            eligible_escapes: resolved.eligible_escapes,
        }
    }
}

/// Rewrites the pre-expansion pass has built, waiting for the late pass
/// to anchor each at its enclosing HIR node and emit. A process-wide
/// static is the same mechanism `overly_long_print_macro` uses to bridge its
/// pre-expansion and late halves; see [`mod@queue`].
static PENDING_VIOLATIONS: Mutex<Vec<PendingViolation>> = Mutex::new(Vec::new());

/// Source byte ranges (`lo`, `hi`) of the cooked string literals the
/// late `check_expr` pass saw in the HIR. The drain at
/// `check_crate_post` skips any queued pre-expansion candidate whose
/// range is in here — those literals survived lowering and the late pass
/// already owns them, so only the *consumed* literals (split format
/// templates, `stringify!` contents, ...) are emitted from the queue.
///
/// Keyed by raw `BytePos` rather than `Span` so the comparison is immune
/// to the macro-hygiene `SyntaxContext` differences between a
/// pre-expansion token span and its post-expansion HIR span; a literal's
/// source byte range uniquely identifies it within one crate's
/// `SourceMap`.
static VISITED_LITERALS: Mutex<BTreeSet<(u32, u32)>> = Mutex::new(BTreeSet::new());

fn queue(violation: PendingViolation) {
    let mut guard = PENDING_VIOLATIONS
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    guard.push(violation);
}

impl_lint_pass!(AvoidableStringEscapes => [AVOIDABLE_STRING_ESCAPES]);
impl_lint_pass!(AvoidableStringEscapesEarly => [AVOIDABLE_STRING_ESCAPES]);

impl Register for AvoidableStringEscapesRule {
    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[AVOIDABLE_STRING_ESCAPES]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        if let DefaultState::Inactive =
            resolved_state("avoidable_string_escapes", DefaultState::Active)
        {
            return;
        }
        // The pre-expansion pass sees every macro's string literals while
        // the source tokens are intact — including the ones lowering would
        // consume before the HIR exists — and parks each rewrite for the late
        // pass to dedup, anchor, and emit. See [`mod@early`].
        lint_store.register_pre_expansion_pass(|| Box::new(AvoidableStringEscapesEarly::new()));
        lint_store.register_late_pass(|_| Box::new(AvoidableStringEscapes::new()));
    }
}

impl<'tcx> LateLintPass<'tcx> for AvoidableStringEscapes {
    fn check_expr(&mut self, lint_context: &LateContext<'tcx>, expr: &Expr<'tcx>) {
        let ExprKind::Lit(literal) = expr.kind else {
            return;
        };
        if !matches!(literal.node, LitKind::Str(_, StrStyle::Cooked)) {
            return;
        }
        // Record that this literal survived into the HIR (so the
        // pre-expansion drain leaves its source range to us) and dedup on
        // the way in: `insert` returns `false` when this exact source
        // range was already visited. That happens when a `macro_rules!`
        // fragment is expanded into several surviving positions — every
        // copy reuses the one call-site span — so the first occurrence
        // owns the diagnostic and the later copies (and the queued
        // candidate at this range) are suppressed. The insert happens
        // before the bails below so an ineligible literal still claims its
        // range against a duplicate queued candidate.
        if !VISITED_LITERALS
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert((literal.span.lo().0, literal.span.hi().0))
        {
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
        let Some(scan) = scan_body(body, &self.eligible_escapes) else {
            return;
        };
        // A literal with zero eliminable escapes is skipped by the
        // threshold itself: `min_escapes_to_trigger: NonZeroUsize`
        // forces the minimum to at least 1, so `count < min` already
        // catches `count == 0`. Suggesting `r"hello"` for `"hello"`
        // would just trip `clippy::needless_raw_strings` on the next
        // pass; the type system now guarantees we never do.
        if scan.eliminable_count < self.min_escapes_to_trigger.get() {
            return;
        }
        span_lint_and_sugg(
            lint_context,
            AVOIDABLE_STRING_ESCAPES,
            literal.span,
            VIOLATION_MESSAGE,
            SUGGESTION_LABEL,
            build_raw_string_suggestion(&scan.decoded),
            Applicability::MachineApplicable,
        );
    }

    /// Drain the pre-expansion pass's queue of rewrites for literals the
    /// HIR walk never reached, and emit each at its deepest enclosing HIR
    /// node — by which point `cfg_attr` has resolved and a per-site
    /// `#[allow]` applies. A candidate is dropped if the late pass already
    /// saw a literal at the same source range (it survived lowering, so
    /// `check_expr` owns it) or if an earlier candidate already covered
    /// that range (the same literal reached through nested macro
    /// invocations).
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let pending: Vec<PendingViolation> = {
            let mut guard = PENDING_VIOLATIONS
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            core::mem::take(&mut *guard)
        };
        let visited: BTreeSet<(u32, u32)> = {
            let mut guard = VISITED_LITERALS
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            core::mem::take(&mut *guard)
        };
        let mut emitted: BTreeSet<(u32, u32)> = BTreeSet::new();
        let surviving: Vec<PendingViolation> = pending
            .into_iter()
            .filter(|violation| {
                let range = (violation.span.lo().0, violation.span.hi().0);
                !visited.contains(&range) && emitted.insert(range)
            })
            .collect();
        if surviving.is_empty() {
            return;
        }
        let target_spans: Vec<_> = surviving.iter().map(|violation| violation.span).collect();
        let best = find_enclosing_hir_ids(lint_context.tcx, &target_spans);
        for (violation, &hir_id) in surviving.into_iter().zip(best.iter()) {
            emit::emit_raw_string(lint_context, hir_id, violation.span, violation.suggestion);
        }
    }
}
