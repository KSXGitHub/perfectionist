//! `perfectionist::unit_test_file_layout` — enforce where a crate's
//! unit-test code lives.
//!
//! Two independent axes are checked:
//!
//! - **External-file layout.** An external `#[cfg(test)] mod <name>;`
//!   must resolve to the canonical on-disk location for the configured
//!   `external_layout`.
//! - **Inline footprint.** Inline test code is summed per source file
//!   and held below the configured budget (or disallowed entirely).
//!
//! Module layout:
//!
//! - [`config`] — the `Config` table, its enums, and the resolved
//!   [`config::UnitTestFileLayout`] pass state.
//! - [`scan`] — the per-crate walk that classifies every item,
//!   accumulates each source file's inline-test footprint, and emits
//!   the inline-style diagnostics.
//! - [`layout`] — the external-module on-disk layout and
//!   unexpected-sibling checks, plus the path arithmetic they share.

use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

use crate::common::{DefaultState, resolved_state};

mod config;
mod layout;
mod scan;

use config::UnitTestFileLayout;

declare_tool_lint! {
    /// ### What it does
    /// Enforces where a crate's unit-test code lives. Two independent
    /// axes are checked:
    ///
    /// 1. **External-file layout.** An external
    ///    `#[cfg(test)] mod <name>;` must resolve to the canonical
    ///    on-disk location. By default that is the nested
    ///    `<parent>/<name>.rs` form (tests of `src/foo.rs` live in
    ///    `src/foo/tests.rs`); for such a file the flattened sibling
    ///    `src/foo_tests.rs` and the skipped-intermediate `src/tests.rs`
    ///    are flagged. A directory-owning parent (`lib.rs` / `main.rs` /
    ///    `mod.rs`) is the exception: its children already live beside
    ///    it, so `mod tests;` in `src/lib.rs` canonically resolves to
    ///    `src/tests.rs` (not `src/lib/tests.rs`) and is *not* flagged —
    ///    matching where Cargo loads it. The `sibling` style also
    ///    accepts the flattened form; `any` skips the layout check.
    /// 2. **Inline footprint.** Inline test code — `#[cfg(test)] mod X
    ///    { ... }` blocks, `#[test] fn`s, `#[cfg(test)] fn` helpers,
    ///    and any other `#[cfg(test)]` item — is summed per file. The
    ///    default `external_when_long` style flags a file once its
    ///    inline-test footprint crosses `inline_max_lines` (or the
    ///    optional `inline_max_fraction_of_file`); `external_only`
    ///    flags every inline test item regardless of length. A file
    ///    whose top-level items are *entirely* test code is exempt — it
    ///    is itself a valid extraction target.
    ///
    /// The module identifier is irrelevant to the layout rule; only the
    /// file's position relative to its parent matters.
    ///
    /// Only the library or binary crate is checked. Integration tests
    /// (`tests/`), benchmarks (`benches/`), and examples (`examples/`)
    /// are separate targets, not the library or binary whose unit-test
    /// layout this rule governs; for those compiled under `cfg(test)`
    /// their top-level `#[test]` functions *are* the target rather than
    /// unit tests misplaced in a production file, so they are left
    /// untouched.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. Both
    /// source projects keep large test suites out of the production
    /// file, so the file an editor tab, a `grep` hit, or a diff shows
    /// is production code rather than a wall of fixtures; and they put
    /// the extracted file in a predictable place so a reader always
    /// knows where a module's tests are. The thresholds and the
    /// nested-vs-sibling choice are deliberately configurable because
    /// the exact budget and directory shape vary by project.
    ///
    /// ### Example
    /// ```text
    /// // Bad (external_layout = "nested")
    /// src/foo.rs         declares  #[cfg(test)] mod tests;
    /// src/foo_tests.rs   holds the test code
    ///
    /// // Good
    /// src/foo.rs         declares  #[cfg(test)] mod tests;
    /// src/foo/tests.rs   holds the test code
    /// ```
    pub perfectionist::UNIT_TEST_FILE_LAYOUT,
    Warn,
    "unit-test code is in the wrong file or exceeds the inline-test budget",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::unit_test_file_layout";

impl_lint_pass!(UnitTestFileLayout => [UNIT_TEST_FILE_LAYOUT]);

/// Register this rule's lint declaration. Paired with [`register_pass`];
/// see the module-level convention documented in `register_lints`.
pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[UNIT_TEST_FILE_LAYOUT]);
}

/// Install this rule's late pass.
///
/// A late pass is required because the only reliable way to tell that
/// an item carries `#[cfg(test)]` is `clippy_utils::is_cfg_test`,
/// which reads the `CfgTrace` attribute rustc leaves behind after
/// configuration — information that needs `TyCtxt` and is unavailable
/// to the pre-/post-expansion AST passes (the raw `#[cfg(test)]`
/// attribute is consumed during configuration). Consequently the rule
/// only sees test code in a build where `cfg(test)` is active, i.e.
/// the unit-test target that `cargo dylint -- --all-targets` checks.
pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("unit_test_file_layout", DefaultState::Active) {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(UnitTestFileLayout::new()));
}

impl<'tcx> LateLintPass<'tcx> for UnitTestFileLayout {
    fn check_crate(&mut self, lint_context: &LateContext<'tcx>) {
        scan::run(self, lint_context);
    }
}
