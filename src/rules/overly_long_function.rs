use crate::code_lines::count_code_lines;
use crate::common::DefaultState;
use crate::measured_fn::measured_fn;
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::source::snippet_opt;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Counts the lines of code in a function or method body and flags
    /// the body when the count is above `max_lines` (default `50`).
    ///
    /// A line counts when it holds anything other than whitespace and
    /// comments, so blank lines, comment-only lines, and the lines a
    /// block comment spans are free. The braces that open and close
    /// the body are not counted. A function produced by a macro is not
    /// measured. A nested function's lines count towards the function
    /// that contains it, since they sit in its body.
    ///
    /// Test code is measured like any other code; set
    /// `test_code_exception` to leave it alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A
    /// function that runs past a screen is read in pieces, and the
    /// reader has to hold the pieces they have scrolled past. A cap on
    /// length is the plainest of the size measures, and the one that
    /// catches a body that is long without being complex — a sequence
    /// of straight-line steps, each of which would read better as a
    /// function named for what it does.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::too_many_lines` (`pedantic`, off by default) measures
    /// the same thing with a default of 100 and no exemption for test
    /// code. This rule exists so a project can hold its functions to
    /// the same limit it holds their complexity and nesting to, with the
    /// same knobs. Enable one or the other, not both.
    ///
    /// ### Example
    ///
    /// **Avoid:** a body that reads, validates, transforms, and writes
    /// in one run of eighty lines.
    ///
    /// **Prefer:** a body of four calls, each to a function that reads,
    /// validates, transforms, or writes.
    pub perfectionist::OVERLY_LONG_FUNCTION,
    Warn,
    "function body has more lines of code than the configured maximum",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::overly_long_function";

/// A body that fits on one screen.
const DEFAULT_MAX_LINES: usize = 50;

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// The most lines of code a function body may have without being
    /// flagged. Defaults to `50`.
    max_lines: usize,
    /// Whether test code is left alone: functions inside a
    /// `#[cfg(test)]` module, `#[test]` functions, and everything in
    /// an integration-test or benchmark target. Defaults to `false`,
    /// so a test is held to the same limit as the code it exercises.
    test_code_exception: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_lines: DEFAULT_MAX_LINES,
            test_code_exception: false,
        }
    }
}

pub struct OverlyLongFunction {
    config: Config,
}

impl_lint_pass!(OverlyLongFunction => [OVERLY_LONG_FUNCTION]);

impl Register for rule::OverlyLongFunction {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[OVERLY_LONG_FUNCTION]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(OverlyLongFunction {
                config: dylint_linting::config_or_default(CONFIG_KEY),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for OverlyLongFunction {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _span: Span,
        def_id: LocalDefId,
    ) {
        let Some(function) = measured_fn(cx, kind, def_id, self.config.test_code_exception) else {
            return;
        };
        let Some(source) = snippet_opt(cx, body.value.span) else {
            return;
        };
        let count = count_code_lines(body_interior(&source));
        if count <= self.config.max_lines {
            return;
        }
        let max = self.config.max_lines;
        let name = function.name;
        let noun = if count == 1 { "line" } else { "lines" };
        let message =
            format!("function `{name}` has {count} {noun} of code, above the limit of {max}");
        span_lint_and_help(
            cx,
            OVERLY_LONG_FUNCTION,
            function.span,
            message,
            None,
            "split the function into smaller ones, each doing one thing",
        );
    }
}

/// The source between a body's outer braces, or the source unchanged
/// when it is not brace-delimited.
fn body_interior(source: &str) -> &str {
    source
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .unwrap_or(source)
}

#[cfg(test)]
mod tests;
