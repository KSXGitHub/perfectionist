use crate::common::{DefaultState, resolved_state};
use crate::rule_index::{ExcessiveInlineTestsRule, Register};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

mod config;
mod paths;
mod scan;

use config::ExcessiveInlineTests;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Caps how much inline unit-test code a production file may carry.
    /// Inline test code — `#[cfg(test)] mod X { ... }` blocks, `#[test]
    /// fn`s, `#[cfg(test)] fn` helpers, and any other `#[cfg(test)]`
    /// item — is summed per file. The default `external_when_long` style
    /// flags a file once its inline-test footprint crosses
    /// `inline_max_lines` (or the optional
    /// `inline_max_fraction_of_file`); `external_only` flags every inline
    /// test item regardless of length. A file whose top-level items are
    /// *entirely* test code is exempt — it is itself a valid extraction
    /// target.
    ///
    /// An external `#[cfg(test)] mod <name>;` (its body in a separate
    /// file) is neutral: it is already extracted, so it neither charges
    /// the footprint nor is otherwise checked. A test module gated by a
    /// compound predicate that still implies test —
    /// `#[cfg(all(test, unix))]`, `#[cfg(all(test, feature = "..."))]` —
    /// is recognised the same as a bare `#[cfg(test)]`, so its external
    /// file is not re-flagged as inline test code in a production file.
    ///
    /// Only the library or binary crate is checked. Integration tests
    /// (`tests/`), benchmarks (`benches/`), and examples (`examples/`)
    /// are separate targets, not the library or binary whose unit-test
    /// footprint this rule governs; for those compiled under `cfg(test)`
    /// their top-level `#[test]` functions *are* the target rather than
    /// unit tests misplaced in a production file, so they are left
    /// untouched.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Keeping
    /// a large test suite out of the production file means the file an
    /// editor tab, a `grep` hit, or a diff shows is production code
    /// rather than a wall of fixtures. The threshold is deliberately
    /// configurable because the exact budget varies by project.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// // File: foo.rs
    /// #[cfg(test)]
    /// mod tests {
    ///     /* ... 200 lines of test code ... */
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// // File: foo.rs
    /// mod tests;
    /// ```
    ///
    /// ```rust,ignore
    /// // File: foo/tests.rs
    /// /* ... 200 lines of test code ... */
    /// ```
    pub perfectionist::EXCESSIVE_INLINE_TESTS,
    Warn,
    "inline test code should be extracted to a separate file",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::excessive_inline_tests";

impl_lint_pass!(ExcessiveInlineTests => [EXCESSIVE_INLINE_TESTS]);

impl Register for ExcessiveInlineTestsRule {
    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[EXCESSIVE_INLINE_TESTS]);
    }

    /// Install this rule's late pass.
    ///
    /// A late pass is required because the only reliable way to tell that
    /// an item carries `#[cfg(test)]` is the `CfgTrace` attribute rustc
    /// leaves behind after configuration, which
    /// `crate::test_code::cfg_predicate_implies_test` reads — information
    /// that needs `TyCtxt` and is unavailable to the pre-/post-expansion
    /// AST passes (the raw `#[cfg(test)]` attribute is consumed during
    /// configuration). Consequently the rule only sees test code in a
    /// build where `cfg(test)` is active, i.e. the unit-test target that
    /// `cargo dylint -- --all-targets` checks.
    fn register_pass(lint_store: &mut LintStore) {
        if let DefaultState::Inactive =
            resolved_state("excessive_inline_tests", DefaultState::Active)
        {
            return;
        }
        lint_store.register_late_pass(|_| Box::new(ExcessiveInlineTests::new()));
    }
}

impl<'tcx> LateLintPass<'tcx> for ExcessiveInlineTests {
    fn check_crate(&mut self, lint_context: &LateContext<'tcx>) {
        scan::run(self, lint_context);
    }
}
