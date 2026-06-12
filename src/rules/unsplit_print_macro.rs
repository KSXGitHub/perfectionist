use crate::common::{DefaultState, resolved_state};
use crate::macro_template::find_template_literal;
use rustc_ast::MacCall;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use std::sync::Mutex;

mod config;
mod emit;
mod late;
mod queue;
mod scan;

use config::UnsplitPrintMacro;
use late::UnsplitPrintMacroLate;
use queue::PendingViolation;
use scan::build_fold_suggestion;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a `println!`-style macro call whose format template
    /// embeds a `\n` newline *and* whose source line is wider than
    /// `max_line_width` display columns, and folds the template across
    /// lines with the backslash-newline continuation escape:
    ///
    /// ```rust,ignore
    /// println!(
    ///     "error: The error was caused by {err_src}\n\
    ///     hint: Run {magic_cmd} to solve the problem",
    /// );
    /// ```
    ///
    /// The rewrite is byte-for-byte output-preserving: every `\n`
    /// stays, and the trailing `\<newline><indent>` continuation
    /// strips exactly the source newline and indentation it adds.
    ///
    /// Eligibility is name-based — a curated list of the macros whose
    /// output is unchanged by the fold (`println!`, `eprintln!`,
    /// `print!`, `eprint!`, `writeln!`, `write!`, and the `log` family
    /// `log!` / `error!` / `warn!` / `info!` / `debug!` / `trace!`),
    /// replaced wholesale via `target_macros`. Macros that *return a
    /// value* (`format!`, `format_args!`) or *terminate* (`panic!`,
    /// `assert!`, the `debug_assert*` family, ...) are deliberately
    /// out of scope.
    ///
    /// A template that is a runtime expression rather than a string
    /// literal, a raw string literal, or a template with no foldable
    /// interior `\n`, is left alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A long
    /// single line whose string already contains `\n` is hard to read
    /// and hard to scan in a diff; folding it at the embedded newlines
    /// lets each output line read as its own source line without
    /// changing a byte of what the program prints.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// println!("error: The error was caused by {err_src}\nhint: Run {magic_cmd} to solve the problem");
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// println!(
    ///     "error: The error was caused by {err_src}\n\
    ///     hint: Run {magic_cmd} to solve the problem",
    /// );
    /// ```
    pub perfectionist::UNSPLIT_PRINT_MACRO,
    Warn,
    "splittable print macro with an embedded-newline template exceeds the configured line width",
    report_in_external_macro: false
}

impl_lint_pass!(UnsplitPrintMacro => [UNSPLIT_PRINT_MACRO]);
impl_lint_pass!(UnsplitPrintMacroLate => [UNSPLIT_PRINT_MACRO]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[UNSPLIT_PRINT_MACRO]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("unsplit_print_macro", DefaultState::Active) {
        return;
    }
    // Same pre-expansion → late split as `macro_trailing_comma` and
    // `impure_macro_argument`: the pre-expansion pass sees the
    // `MacCall` tokens and builds the rewrite from source, then parks
    // it; the late pass walks the HIR and emits each at its deepest
    // enclosing node, by which point `cfg_attr` has resolved and
    // lint-level attributes apply.
    lint_store.register_pre_expansion_pass(|| Box::new(UnsplitPrintMacro::new()));
    lint_store.register_late_pass(|_| Box::new(UnsplitPrintMacroLate));
}

/// Rewrites the pre-expansion pass has built, waiting for the late pass
/// to anchor each at its enclosing HIR node and emit. See
/// [`mod@queue`] for why a process-wide static is safe.
static PENDING_VIOLATIONS: Mutex<Vec<PendingViolation>> = Mutex::new(Vec::new());

impl EarlyLintPass for UnsplitPrintMacro {
    fn check_mac(&mut self, lint_context: &EarlyContext<'_>, mac_call: &MacCall) {
        if !self.should_check_path(&mac_call.path) {
            return;
        }
        let Some(template_span) = find_template_literal(&mac_call.args.tokens) else {
            return;
        };
        let source_map = lint_context.sess().source_map();
        let Some((call_span, replacement)) =
            build_fold_suggestion(source_map, mac_call, template_span, self.max_line_width())
        else {
            return;
        };
        queue(PendingViolation {
            call_span,
            replacement,
        });
    }
}

fn queue(violation: PendingViolation) {
    let mut guard = PENDING_VIOLATIONS
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    guard.push(violation);
}
