use std::path::Path;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{BytePos, FileName, RealFileName, Span, SyntaxContext, def_id::LOCAL_CRATE};

declare_tool_lint! {
    /// ### What it does
    /// Forbids the `module/mod.rs` layout for submodules. Each
    /// submodule should be defined by a sibling file named after
    /// the module (`module.rs`), with any nested children placed
    /// inside the `module/` directory next to it.
    ///
    /// ### Why is this bad?
    /// The flat layout keeps the file name unique to its module,
    /// so editors, terminal tabs, and `grep` results identify the
    /// module without their parent directory. The `mod.rs` form
    /// produces dozens of identically-named tabs in editors that
    /// don't disambiguate by directory.
    ///
    /// ### Example
    /// ```text
    /// // Bad
    /// src/foo/mod.rs
    ///
    /// // Good
    /// src/foo.rs
    /// src/foo/bar.rs
    /// ```
    pub perfectionist::FLAT_MODULE_PATTERN,
    Warn,
    "submodule defined as `module/mod.rs`; prefer the flat `module.rs` layout",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::flat_module_pattern";

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {}

pub struct FlatModulePattern;

impl FlatModulePattern {
    fn new() -> Self {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self
    }
}

impl_lint_pass!(FlatModulePattern => [FLAT_MODULE_PATTERN]);

/// Register this rule's lint declaration. Paired with [`register_pass`];
/// see the module-level convention documented in `register_lints`.
pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[FLAT_MODULE_PATTERN]);
}

/// Install this rule's late pass.
pub fn register_pass(lint_store: &mut LintStore) {
    lint_store.register_late_pass(|_| Box::new(FlatModulePattern::new()));
}

impl<'tcx> LateLintPass<'tcx> for FlatModulePattern {
    fn check_crate(&mut self, lint_context: &LateContext<'tcx>) {
        let crate_root = lint_context.sess().local_crate_source_file();
        let crate_root_path = crate_root.as_ref().and_then(RealFileName::local_path);
        let source_map = lint_context.sess().source_map();
        for source_file in source_map.files().iter() {
            if source_file.cnum != LOCAL_CRATE {
                continue;
            }
            let FileName::Real(real_file_name) = &source_file.name else {
                continue;
            };
            let Some(path) = real_file_name.local_path() else {
                continue;
            };
            if !is_mod_rs(path) {
                continue;
            }
            if Some(path) == crate_root_path {
                continue;
            }
            let span_start = source_file.start_pos;
            let span = Span::new(
                span_start,
                BytePos(span_start.0),
                SyntaxContext::root(),
                None,
            );
            span_lint_and_help(
                lint_context,
                FLAT_MODULE_PATTERN,
                span,
                "submodule uses the `mod.rs` layout",
                None,
                "rename `mod.rs` to the sibling `<parent>.rs` form",
            );
        }
    }
}

fn is_mod_rs(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "mod.rs")
}
