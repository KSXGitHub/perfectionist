use clippy_utils::macros::root_macro_call_first_node;
use clippy_utils::res::MaybeDef;
use rustc_ast::LitKind;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::sym;

use crate::common::{DefaultState, resolved_state};

mod config;
mod scan;

use config::UnicodeEllipsisInPanicMessages;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Forbids U+2026 HORIZONTAL ELLIPSIS (`…`) in the message of a
    /// panic-family or assertion-style macro (`panic!`,
    /// `unimplemented!`, `todo!`, `unreachable!`, `assert!`,
    /// `assert_eq!`, `assert_ne!`, `debug_assert*!`) and in the
    /// `expect` / `expect_err` argument on `Option` and `Result`.
    /// Prefer the three-ASCII-dot form `...`.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue.
    /// Panic and assertion messages surface in stderr, CI logs, crash
    /// reporters, and on terminals whose locale or encoding may not
    /// be UTF-8. ASCII `...` renders identically everywhere.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// panic!("could not parse manifest…");
    /// let manifest = load().expect("config missing…");
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// panic!("could not parse manifest...");
    /// let manifest = load().expect("config missing...");
    /// ```
    ///
    /// ### Custom macros
    ///
    /// The `extra_macros` configuration accepts any macro name,
    /// but the lint's per-macro knowledge of which argument is
    /// the message only covers the built-in panic / assertion
    /// macros. A custom macro added through this knob is treated
    /// as if its first argument were the message; an
    /// `assert_eq!`-shaped wrapper would therefore also scan its
    /// value-position literals. Adding per-macro skip counts
    /// requires extending the configuration schema and is out of
    /// scope for the initial rule.
    #[cfg_attr(
        dylint_lib = "perfectionist",
        expect(
            perfectionist::unicode_ellipsis_in_docs,
            reason = "this rule's own rustdoc names the U+2026 glyph it governs"
        )
    )]
    pub perfectionist::UNICODE_ELLIPSIS_IN_PANIC_MESSAGES,
    Warn,
    "U+2026 HORIZONTAL ELLIPSIS in panic / assertion / expect messages; prefer `...`",
    // Load-bearing: the user-supplied literal inside `panic!`,
    // `assert!`, `assert_eq!`, etc. lives inside a `core` macro
    // expansion. With the default `false` rustc would treat every
    // diagnostic on those literals as "in an external macro" and
    // drop it before reaching the user.
    report_in_external_macro: true
}

impl_lint_pass!(UnicodeEllipsisInPanicMessages => [UNICODE_ELLIPSIS_IN_PANIC_MESSAGES]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[UNICODE_ELLIPSIS_IN_PANIC_MESSAGES]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive =
        resolved_state("unicode_ellipsis_in_panic_messages", DefaultState::Active)
    {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(UnicodeEllipsisInPanicMessages::new()));
}

impl<'tcx> LateLintPass<'tcx> for UnicodeEllipsisInPanicMessages {
    fn check_expr(&mut self, lint_context: &LateContext<'tcx>, expr: &Expr<'tcx>) {
        // Panic / assertion macros: `root_macro_call_first_node`
        // returns `Some` exactly when `expr` is the boundary HIR
        // node of the outermost macro expansion, so each call's
        // source is scanned once.
        if let Some(macro_call) = root_macro_call_first_node(lint_context, expr) {
            let macro_name = lint_context.tcx.item_name(macro_call.def_id);
            if self.macros.contains(&macro_name) {
                scan::scan_macro_call_source(
                    lint_context,
                    &self.flagged_chars,
                    macro_call.span,
                    macro_name,
                );
            }
        }
        // `expect` / `expect_err` on `Option` / `Result`.
        if let ExprKind::MethodCall(path_segment, receiver, arguments, _) = expr.kind
            && self.methods.contains(&path_segment.ident.name)
            && receiver_is_option_or_result(lint_context, receiver)
            && let Some(message_argument) = arguments.first()
            && let ExprKind::Lit(literal) = message_argument.kind
            && matches!(literal.node, LitKind::Str(..))
        {
            let context = format!("`{}` message", path_segment.ident.name);
            if let Ok(snippet) = lint_context
                .sess()
                .source_map()
                .span_to_snippet(literal.span)
            {
                scan::scan_literal(
                    lint_context,
                    &self.flagged_chars,
                    literal.span,
                    &snippet,
                    &context,
                );
            }
        }
    }
}

fn receiver_is_option_or_result<'tcx>(
    lint_context: &LateContext<'tcx>,
    receiver: &Expr<'tcx>,
) -> bool {
    let receiver_type = lint_context.typeck_results().expr_ty(receiver).peel_refs();
    receiver_type.is_diag_item(lint_context, sym::Option)
        || receiver_type.is_diag_item(lint_context, sym::Result)
}
