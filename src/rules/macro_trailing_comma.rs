use std::sync::Mutex;

use rustc_ast::MacCall;
use rustc_ast::token::TokenKind;
use rustc_ast::tokenstream::TokenTree;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

use crate::common::{DefaultState, resolved_state};

mod config;
mod emit;
mod late;
mod queue;

use config::MacroTrailingComma;
use late::MacroTrailingCommaLate;
use queue::PendingViolation;

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
    /// `anyhow::bail!`, `maplit::hashmap!`, ...), extended via
    /// `extra_macros` and overridden via `ignore`.
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
    /// Multi-line invocations whose first top-level token starts on the
    /// opening-delimiter line (visual-indent / compact layout, e.g.
    /// `vec![Inner { ... }]`) are skipped: rustfmt's `Vertical` policy
    /// only adds a trailing comma when each top-level item is on its
    /// own line, separate from the delimiter, and strips any comma
    /// added to the compact shape. The two tools have to agree.
    ///
    /// ### Example
    /// **Bad:**
    /// ```rust,ignore
    /// let xs = vec![
    ///     1,
    ///     2,
    ///     3
    /// ];
    /// let ys = vec![1, 2, 3,];
    /// ```
    /// **Good:**
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

impl_lint_pass!(MacroTrailingComma => [MACRO_TRAILING_COMMA]);
impl_lint_pass!(MacroTrailingCommaLate => [MACRO_TRAILING_COMMA]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[MACRO_TRAILING_COMMA]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("macro_trailing_comma", DefaultState::Active) {
        return;
    }
    // Split across two passes per
    // <https://github.com/KSXGitHub/parallel-disk-usage/issues/409>:
    // pre-expansion sees the `MacCall` tokens but runs before
    // `cfg_attr` is evaluated, so a `cfg_attr`-wrapped `#[expect]`
    // is invisible at emission time. The pre-expansion pass parks
    // violation spans in `PENDING_VIOLATIONS`; the late pass walks
    // the HIR and emits each at its deepest enclosing node, by which
    // point `cfg_attr` has resolved and lint-level attributes apply.
    lint_store.register_pre_expansion_pass(|| Box::new(MacroTrailingComma::new()));
    lint_store.register_late_pass(|_| Box::new(MacroTrailingCommaLate));
}

/// Violations the pre-expansion pass has found, waiting to be emitted
/// by the late pass at the appropriate HIR node. Spans are `Copy +
/// Send + Sync` (they're 32-bit ids into a session-side table), so
/// stashing them in a process-wide static is safe.
static PENDING_VIOLATIONS: Mutex<Vec<PendingViolation>> = Mutex::new(Vec::new());

impl EarlyLintPass for MacroTrailingComma {
    fn check_mac(&mut self, lint_context: &EarlyContext<'_>, mac_call: &MacCall) {
        if !self.should_check_path(&mac_call.path) {
            return;
        }
        check_invocation(lint_context, mac_call);
    }
}

fn check_invocation(lint_context: &EarlyContext<'_>, mac_call: &MacCall) {
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

fn queue(violation: PendingViolation) {
    let mut guard = PENDING_VIOLATIONS
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    guard.push(violation);
}
