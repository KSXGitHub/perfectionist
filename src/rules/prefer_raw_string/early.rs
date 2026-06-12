//! Pre-expansion pass that rescues string literals the late
//! `ExprKind::Lit` pass cannot see.
//!
//! Some macros consume a string literal during expansion so that it
//! never survives into the HIR as one `LitKind::Str` node: a `format!`
//! template carrying a `{...}` placeholder is split into its static
//! pieces, `stringify!`/`dbg!` reflect it as source text, and so on. The
//! late pass, which only walks the HIR, can't reach those.
//!
//! This pass runs before expansion, where the macro's source tokens are
//! intact, and collects *every* cooked string literal in every macro
//! invocation (via [`crate::macro_template::find_all_cooked_str_literals`]).
//! The raw-string rewrite is value-preserving for any literal, so there
//! is no need to single out "the template" or restrict to a macro
//! allowlist. Each eligible literal is queued; the late pass drains the
//! queue at `check_crate_post`, **skipping any literal it already saw in
//! the HIR** (recorded by byte range — see [`super::VISITED_LITERALS`]).
//! That dedup is what keeps the two passes disjoint: literals that
//! survive lowering belong to the late pass, and only the consumed ones
//! are emitted here.

use super::parser::{build_raw_string_suggestion, scan_body};
use super::queue::PendingViolation;
use super::resolved_config;
use crate::macro_template::find_all_cooked_str_literals;
use core::num::NonZeroUsize;
use rustc_ast::MacCall;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};

pub(super) struct PreferRawStringEarly {
    min_escapes_to_trigger: NonZeroUsize,
    eligible_escapes: Vec<String>,
}

impl PreferRawStringEarly {
    pub(super) fn new() -> Self {
        let resolved = resolved_config();
        Self {
            min_escapes_to_trigger: resolved.min_escapes_to_trigger,
            eligible_escapes: resolved.eligible_escapes,
        }
    }
}

impl EarlyLintPass for PreferRawStringEarly {
    fn check_mac(&mut self, lint_context: &EarlyContext<'_>, mac_call: &MacCall) {
        let source_map = lint_context.sess().source_map();
        for literal_span in find_all_cooked_str_literals(&mac_call.args.tokens) {
            let Ok(snippet) = source_map.span_to_snippet(literal_span) else {
                continue;
            };
            // Synthesised spans and any non-cooked spelling that slips
            // through bail here, mirroring the late pass's belt-and-braces
            // quote-stripping guard.
            let Some(body) = snippet
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
            else {
                continue;
            };
            let Some(scan) = scan_body(body, &self.eligible_escapes) else {
                continue;
            };
            if scan.eliminable_count < self.min_escapes_to_trigger.get() {
                continue;
            }
            // Qualified rather than imported: the parent module also
            // has a `queue` submodule (used above for `PendingViolation`),
            // and naming the function through `super::` keeps the two
            // `queue` spellings visibly distinct at a glance.
            super::queue(PendingViolation {
                span: literal_span,
                suggestion: build_raw_string_suggestion(&scan.decoded),
            });
        }
    }
}
