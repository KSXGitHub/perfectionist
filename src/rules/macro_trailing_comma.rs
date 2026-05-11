use std::collections::BTreeSet;
use std::sync::Mutex;

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_ast::MacCall;
use rustc_ast::token::TokenKind;
use rustc_ast::tokenstream::TokenTree;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_hir::intravisit::{self, Visitor};
use rustc_lint::{EarlyContext, EarlyLintPass, LateContext, LateLintPass, LintContext, LintStore};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;
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
    /// Multi-line invocations whose argument list is a single element
    /// laid out in visual-indent style — the element starts on the
    /// opening-delimiter line, e.g. `vec![Inner { ... }]` — are skipped.
    /// rustfmt's `Vertical` policy treats those as compact rather than
    /// block-indent and leaves the trailing comma off (and strips any
    /// that gets added), so the two tools have to agree.
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
    enabled: bool,
    /// Accepted for forward compatibility with the matcher-based half of
    /// the rule. Currently a no-op — only name-based eligibility is
    /// implemented; see `planned-rules/macro-trailing-comma.md` for the
    /// status breakdown.
    matcher_based: bool,
    extra_name_based: Vec<String>,
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
impl_lint_pass!(MacroTrailingCommaLate => [MACRO_TRAILING_COMMA]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[MACRO_TRAILING_COMMA]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    // The lint is split across two passes for issue #409: a
    // `cfg_attr`-wrapped `#[expect]` is not yet visible to a pre-
    // expansion lint level lookup (the `cfg_attr` is itself an
    // attribute that has not been evaluated yet), so emitting the
    // diagnostic during pre-expansion would slip past rustc's
    // expectation tracker and the expectation would be reported as
    // unfulfilled.
    //
    // The pre-expansion pass collects violation spans into
    // `PENDING_VIOLATIONS`; the late pass walks the HIR to find the
    // deepest containing node for each span and emits the diagnostic
    // there. By that point `cfg_attr` has been evaluated, so the
    // `#[expect]` / `#[allow]` attributes the user wrote on the crate
    // root (or any enclosing item) are visible to the lint-level
    // lookup and behave correctly.
    //
    // Pre-expansion remains necessary for the first half: post-
    // expansion the macros covered by this rule have been expanded
    // away and `check_mac` would never fire.
    lint_store.register_pre_expansion_pass(|| Box::new(MacroTrailingComma::new()));
    lint_store.register_late_pass(|_| Box::new(MacroTrailingCommaLate));
}

/// Violations the pre-expansion pass has found, waiting to be emitted
/// by the late pass at the appropriate HIR node. Spans are `Copy +
/// Send + Sync` (they're 32-bit ids into a session-side table), so
/// stashing them in a process-wide static is safe.
static PENDING_VIOLATIONS: Mutex<Vec<PendingViolation>> = Mutex::new(Vec::new());

#[derive(Debug, Clone, Copy)]
enum PendingViolation {
    /// Multi-line macro invocation, missing a trailing comma after the
    /// last argument. The span is the insertion point.
    Insert(Span),
    /// Single-line macro invocation, with a gratuitous trailing comma
    /// after the last argument. The span covers the comma to remove.
    Remove(Span),
}

impl PendingViolation {
    fn span(self) -> Span {
        match self {
            PendingViolation::Insert(span) | PendingViolation::Remove(span) => span,
        }
    }
}

pub struct MacroTrailingCommaLate;

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
        // first and last trees and bail on a top-level `;`. Avoids
        // allocating a `Vec` per `check_mac` call.
        let mut first_tree: Option<&TokenTree> = None;
        let mut last_tree: Option<&TokenTree> = None;
        for tree in args.tokens.iter() {
            if let TokenTree::Token(token, _) = tree
                && token.kind == TokenKind::Semi
            {
                return;
            }
            if first_tree.is_none() {
                first_tree = Some(tree);
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
        // rustfmt's `trailing_comma = "Vertical"` only applies in block-
        // indent style -- i.e., when the first top-level item starts on
        // a line of its own, separate from the opening delimiter. If
        // the first item shares its starting line with the opening
        // delimiter (the compact / visual-indent layout that rustfmt
        // produces for a single multi-line element such as
        // `vec![Inner { ... }]` or `vec![bar(\n    ...,\n)]`), rustfmt
        // leaves the trailing comma off and actively strips any that
        // gets added. Skip the multi-line "insert comma" branch in that
        // case so the two tools agree.
        let suppress_insert = is_multi_line
            && first_tree.is_some_and(|first_tree| {
                let between = args.dspan.open.between(first_tree.span());
                !source_map.is_multiline(between)
            });
        match (is_multi_line, last_is_comma) {
            (true, false) if !suppress_insert => {
                queue(PendingViolation::Insert(last_tree.span().shrink_to_hi()));
            }
            (false, true) => queue(PendingViolation::Remove(last_tree.span())),
            _ => {}
        }
    }
}

fn queue(violation: PendingViolation) {
    let mut guard = PENDING_VIOLATIONS
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    guard.push(violation);
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

impl<'tcx> LateLintPass<'tcx> for MacroTrailingCommaLate {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        let pending: Vec<PendingViolation> = {
            let mut guard = PENDING_VIOLATIONS
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            std::mem::take(&mut *guard)
        };
        if pending.is_empty() {
            return;
        }
        let tcx = lint_context.tcx;
        let mut best: Vec<hir::HirId> = vec![hir::CRATE_HIR_ID; pending.len()];
        let mut finder = EnclosingHirFinder {
            tcx,
            pending: &pending,
            best: &mut best,
        };
        tcx.hir_walk_toplevel_module(&mut finder);
        for (violation, &hir_id) in pending.iter().zip(best.iter()) {
            match *violation {
                PendingViolation::Insert(span) => emit_insert(lint_context, hir_id, span),
                PendingViolation::Remove(span) => emit_remove(lint_context, hir_id, span),
            }
        }
    }
}

/// Walk the HIR once and, for each pending violation span, record the
/// deepest HIR node whose span contains it. Emitting at that node ties
/// the diagnostic to the user's lint-level attributes — both on the
/// containing item and any ancestor — so `#[allow]` / `#[expect]` at
/// any scope behave correctly.
struct EnclosingHirFinder<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    pending: &'a [PendingViolation],
    best: &'a mut [hir::HirId],
}

impl<'a, 'tcx> EnclosingHirFinder<'a, 'tcx> {
    fn update(&mut self, hir_id: hir::HirId, span: Span) {
        for (index, violation) in self.pending.iter().enumerate() {
            let target = violation.span();
            if !span.contains(target) {
                continue;
            }
            // The walk is depth-first: a parent is visited before its
            // children, so each successful containment update lands on
            // the deepest node seen so far.
            self.best[index] = hir_id;
        }
    }
}

impl<'tcx> Visitor<'tcx> for EnclosingHirFinder<'_, 'tcx> {
    type NestedFilter = nested_filter::All;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) {
        self.update(item.hir_id(), item.span);
        intravisit::walk_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'tcx hir::TraitItem<'tcx>) {
        self.update(item.hir_id(), item.span);
        intravisit::walk_trait_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'tcx hir::ImplItem<'tcx>) {
        self.update(item.hir_id(), item.span);
        intravisit::walk_impl_item(self, item);
    }

    fn visit_foreign_item(&mut self, item: &'tcx hir::ForeignItem<'tcx>) {
        self.update(item.hir_id(), item.span);
        intravisit::walk_foreign_item(self, item);
    }

    fn visit_block(&mut self, block: &'tcx hir::Block<'tcx>) {
        self.update(block.hir_id, block.span);
        intravisit::walk_block(self, block);
    }

    fn visit_stmt(&mut self, stmt: &'tcx hir::Stmt<'tcx>) {
        self.update(stmt.hir_id, stmt.span);
        intravisit::walk_stmt(self, stmt);
    }

    fn visit_local(&mut self, local: &'tcx hir::LetStmt<'tcx>) {
        self.update(local.hir_id, local.span);
        intravisit::walk_local(self, local);
    }

    fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        self.update(expr.hir_id, expr.span);
        intravisit::walk_expr(self, expr);
    }

    fn visit_pat(&mut self, pat: &'tcx hir::Pat<'tcx>) {
        self.update(pat.hir_id, pat.span);
        intravisit::walk_pat(self, pat);
    }
}

fn emit_insert(lint_context: &LateContext<'_>, hir_id: hir::HirId, insert_at: Span) {
    span_lint_hir_and_then(
        lint_context,
        MACRO_TRAILING_COMMA,
        hir_id,
        insert_at,
        "multi-line macro invocation should end with a trailing comma",
        |diag| {
            diag.span_suggestion(
                insert_at,
                "add a trailing comma",
                ",",
                Applicability::MachineApplicable,
            );
        },
    );
}

fn emit_remove(lint_context: &LateContext<'_>, hir_id: hir::HirId, comma_span: Span) {
    span_lint_hir_and_then(
        lint_context,
        MACRO_TRAILING_COMMA,
        hir_id,
        comma_span,
        "single-line macro invocation should not end with a trailing comma",
        |diag| {
            diag.span_suggestion(
                comma_span,
                "remove the trailing comma",
                "",
                Applicability::MachineApplicable,
            );
        },
    );
}
