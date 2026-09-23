use crate::code_lines::count_code_lines_excluding;
use crate::common::{DefaultState, plural};
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_hir::HirId;
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::SourceFile;
use std::{collections::HashSet, sync::Arc};

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
    /// measured like any other; set `exempt_tests` to leave it
    /// alone.
    ///
    /// Set `exclude_imports` to avoid counting lines belonging to top-level
    /// `use` items. This is useful when a project chooses a vertical import
    /// layout, where formatting can add several lines without adding code.
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
    /// and the validation, in 2000 lines.
    ///
    /// **Prefer:** `src/config.rs` declaring the struct and
    /// `pub mod env; pub mod json; pub mod toml; pub mod validate;
    /// pub mod yaml;`, each a file a reader can take in whole.
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
    exempt_tests: bool,
    /// Whether lines belonging to top-level `use` items are excluded.
    /// Defaults to `false`.
    exclude_imports: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_lines: DEFAULT_MAX_LINES,
            exempt_tests: false,
            exclude_imports: false,
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
        if self.config.exempt_tests && item_in_test_code(cx, hir_id.expect_owner().def_id) {
            return;
        }
        let Some(source) = file.src.as_deref() else {
            return;
        };
        let empty = HashSet::new();
        let excluded_lines = self
            .config
            .exclude_imports
            .then(|| import_lines(&file, module, cx));
        let count = count_code_lines_excluding(source, excluded_lines.as_ref().unwrap_or(&empty));
        if count <= self.config.max_lines {
            return;
        }
        let max = self.config.max_lines;
        let name = cx.sess().source_map().filename_for_diagnostics(&file.name);
        let noun = plural(count, "line", "lines");
        let message = format!("file `{name}` has {count} {noun} of code, above the limit of {max}");
        span_lint_and_help(
            cx,
            OVERLY_LONG_FILE,
            module.spans.inner_span.shrink_to_lo(),
            message,
            None,
            "split the file into modules: name each for what it holds, not where the \
             file was cut; if each needs most of the other's items, the lines moved \
             rather than the concerns separated",
        );
    }
}

fn import_lines(file: &SourceFile, module: &hir::Mod<'_>, cx: &LateContext<'_>) -> HashSet<usize> {
    let Some(source) = file.src.as_deref() else {
        return HashSet::new();
    };
    let mut lines = HashSet::new();
    for item_id in module.item_ids {
        let item = cx.tcx.hir_item(*item_id);
        if !matches!(item.kind, hir::ItemKind::Use(..)) {
            continue;
        }
        let Some(start) = item.span.lo().0.checked_sub(file.start_pos.0) else {
            continue;
        };
        let Some(end) = item.span.hi().0.checked_sub(file.start_pos.0) else {
            continue;
        };
        let start = start as usize;
        let end = (end as usize).min(source.len());
        if start >= end || start >= source.len() {
            continue;
        }
        let first_line = source[..start]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count();
        let last_line = first_line
            + source[start..end]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count();
        lines.extend(first_line..=last_line);
    }
    lines
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
