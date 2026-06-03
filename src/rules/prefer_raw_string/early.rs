//! Pre-expansion pass that rescues `format!`-family templates the late
//! `ExprKind::Lit` pass cannot see.
//!
//! A `format!`-family template that contains a `{...}` placeholder is
//! lowered through `rustc_ast::FormatArgs`: by the time the HIR exists
//! the template has been split into its static pieces and arguments, so
//! it never survives as one `LitKind::Str` node for the late pass to
//! visit. A placeholder-free template, by contrast, lowers to a single
//! `&str` literal and is handled by the late pass as normal.
//!
//! This pass closes that gap. It runs before expansion, where the
//! macro's source tokens are intact, finds the format template via
//! [`crate::macro_template::find_template_literal`], and — *only* when
//! the template carries a real placeholder (the exact case the late
//! pass misses, which keeps the two passes from both firing on one
//! literal) — scans it with the same eligible-escape parser the late
//! pass uses and queues a raw-string rewrite. The queued rewrite is
//! emitted by the late pass's `check_crate_post`, anchored at the
//! enclosing HIR node so a `cfg_attr`-wrapped `#[allow]` resolves.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use rustc_ast::MacCall;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};

use super::parser::{build_raw_string_suggestion, scan_body};
use super::queue::PendingViolation;
use super::{queue, resolved_config};
use crate::macro_path::{matches_any, merge_with_builtins};
use crate::macro_template::find_template_literal;

/// `format!`-family macros whose template argument is routed through
/// `format_args!` and therefore split away from the HIR once it carries
/// a placeholder. Matched by final path segment, so `format` covers
/// `std::format!` / `core::format!`, and `info` covers `log::info!` /
/// `tracing::info!`.
///
/// The `assert_eq!` / `assert_ne!` family is deliberately absent: their
/// leading arguments are *compared values* that may themselves be lone
/// string literals, which the "first lone cooked literal" template
/// search would misread as the message template. Their message argument
/// is reachable, but pinning it down robustly is out of scope for this
/// false-negative fix.
const FORMAT_TEMPLATE_MACROS: &[&str] = &[
    "format",
    "format_args",
    "print",
    "println",
    "eprint",
    "eprintln",
    "write",
    "writeln",
    "panic",
    "todo",
    "unimplemented",
    "unreachable",
    "assert",
    "debug_assert",
    "log",
    "error",
    "warn",
    "info",
    "debug",
    "trace",
];

pub(super) struct PreferRawStringEarly {
    min_escapes_to_trigger: NonZeroUsize,
    eligible_escapes: Vec<String>,
    target_macros: BTreeSet<Vec<String>>,
}

impl PreferRawStringEarly {
    pub(super) fn new() -> Self {
        let resolved = resolved_config();
        Self {
            min_escapes_to_trigger: resolved.min_escapes_to_trigger,
            eligible_escapes: resolved.eligible_escapes,
            target_macros: merge_with_builtins(FORMAT_TEMPLATE_MACROS, &BTreeSet::new()),
        }
    }
}

impl EarlyLintPass for PreferRawStringEarly {
    fn check_mac(&mut self, lint_context: &EarlyContext<'_>, mac_call: &MacCall) {
        if !matches_any(&mac_call.path, &self.target_macros) {
            return;
        }
        let Some(template_span) = find_template_literal(&mac_call.args.tokens) else {
            return;
        };
        let source_map = lint_context.sess().source_map();
        let Ok(snippet) = source_map.span_to_snippet(template_span) else {
            return;
        };
        // Synthesised spans and any non-cooked spelling that slips
        // through bail here, mirroring the late pass's belt-and-braces
        // quote-stripping guard.
        let Some(body) = snippet
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
        else {
            return;
        };
        let Some(scan) = scan_body(body, &self.eligible_escapes) else {
            return;
        };
        // Only placeholder-carrying templates are split away from the
        // HIR; a placeholder-free one survives as a single literal and
        // is the late pass's job. Skipping it here is what keeps the two
        // passes from both flagging the same literal.
        if !has_format_placeholder(&scan.decoded) {
            return;
        }
        if scan.eliminable_count < self.min_escapes_to_trigger.get() {
            return;
        }
        queue(PendingViolation {
            span: template_span,
            suggestion: build_raw_string_suggestion(&scan.decoded),
        });
    }
}

/// Whether a format-string body contains a real `{...}` placeholder, as
/// opposed to only the escaped braces `{{` / `}}`. Switching to a raw
/// string leaves placeholder parsing untouched (`{` is still a
/// placeholder, `{{` still an escaped brace), so this only gates *which
/// pass* handles the literal — it never changes the rewrite.
///
/// Braces are ASCII, so a byte scan is correct over UTF-8 text; and a
/// `\` never escapes a brace in a Rust string literal, so the decoded
/// body carries the same braces as the source.
fn has_format_placeholder(body: &str) -> bool {
    let bytes = body.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'{' if bytes.get(index + 1) == Some(&b'{') => index += 2,
            b'{' => return true,
            b'}' if bytes.get(index + 1) == Some(&b'}') => index += 2,
            _ => index += 1,
        }
    }
    false
}
