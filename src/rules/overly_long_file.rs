use crate::code_lines::count_code_lines;
use crate::common::DefaultState;
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_hir::HirId;
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::SourceFile;
use std::sync::Arc;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Counts the lines of code in each source file of the crate — the
    /// crate root and every `mod name;` that lives in a file of its own
    /// — and flags a file with more than `max_lines` (default `500`).
    ///
    /// A line counts when it holds anything other than whitespace and
    /// comments, so blank lines, comment-only lines, doc comments, and
    /// the lines a block comment spans are free. An inline
    /// `mod name { ... }` is part of the file that holds it, not a file
    /// of its own.
    ///
    /// A file of test code — a `mod tests;` behind `#[cfg(test)]`, or
    /// any file of an integration-test or benchmark target — is
    /// measured like any other; set `test_code_exception` to leave it
    /// alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A file
    /// is the unit a reader opens, searches, and scrolls; when one
    /// grows past a few hundred lines it is holding more than one
    /// concern, and the reader has to find the boundaries between them
    /// that a module split would have drawn. The cap pushes a growing
    /// file to become a directory of files named for what each does,
    /// and it stops the next addition from landing wherever the file
    /// happened to be open.
    ///
    /// ### Example
    ///
    /// **Avoid:** `src/config.rs` holding the settings struct, the
    /// parser for each of three file formats, the environment overlay,
    /// and the validation, in two thousand lines.
    ///
    /// **Prefer:** `src/config.rs` declaring the struct and
    /// `pub mod env; pub mod validate; pub mod yaml;`, each a file a
    /// reader can take in whole.
    pub perfectionist::OVERLY_LONG_FILE,
    Warn,
    "source file has more lines of code than the configured maximum",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::overly_long_file";

/// A file a reader can still hold as one thing.
const DEFAULT_MAX_LINES: usize = 500;

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// The most lines of code a file may have without being flagged.
    /// Defaults to `500`.
    max_lines: usize,
    /// Whether files of test code are left alone: a module behind
    /// `#[cfg(test)]`, and every file of an integration-test or
    /// benchmark target. Defaults to `false`, so a test file is held
    /// to the same limit as the code it exercises.
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

pub struct OverlyLongFile {
    config: Config,
}

impl_lint_pass!(OverlyLongFile => [OVERLY_LONG_FILE]);

impl Register for rule::OverlyLongFile {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[OVERLY_LONG_FILE]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(OverlyLongFile {
                config: dylint_linting::config_or_default(CONFIG_KEY),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for OverlyLongFile {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx hir::Mod<'tcx>, hir_id: HirId) {
        let Some(file) = own_file(cx, module, hir_id) else {
            return;
        };
        if self.config.test_code_exception && item_in_test_code(cx, hir_id.expect_owner().def_id) {
            return;
        }
        let Some(source) = file.src.as_deref() else {
            return;
        };
        let count = count_code_lines(source);
        if count <= self.config.max_lines {
            return;
        }
        let max = self.config.max_lines;
        let name = cx.sess().source_map().filename_for_diagnostics(&file.name);
        let message = format!("file `{name}` has {count} lines of code, above the limit of {max}");
        span_lint_and_help(
            cx,
            OVERLY_LONG_FILE,
            module.spans.inner_span.shrink_to_lo(),
            message,
            None,
            "split the module into submodules, one concern each",
        );
    }
}

/// The file `module` is the whole of: the crate root's file, or the
/// file an out-of-line `mod name;` names. `None` for an inline module,
/// whose contents belong to the file that declares it.
fn own_file(cx: &LateContext<'_>, module: &hir::Mod<'_>, hir_id: HirId) -> Option<Arc<SourceFile>> {
    let source_map = cx.sess().source_map();
    let inner_file = source_map.lookup_source_file(module.spans.inner_span.lo());
    if hir_id == hir::CRATE_HIR_ID {
        return Some(inner_file);
    }
    let declaration_file = source_map.lookup_source_file(cx.tcx.hir_span(hir_id).lo());
    (!Arc::ptr_eq(&inner_file, &declaration_file)).then_some(inner_file)
}
