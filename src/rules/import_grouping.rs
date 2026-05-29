use clippy_utils::diagnostics::span_lint_hir_and_then;
use clippy_utils::source::indent_of;
use rustc_ast::{Item, ItemKind, ModKind};
use rustc_errors::Applicability;
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{BytePos, Span, sym};

mod check;
mod classify;
mod config;
mod render;

use config::{Config, Style};

use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::find_enclosing_hir_ids;
use crate::module_reparse::parse_crate_module_files;

declare_tool_lint! {
    /// ### What it does
    /// Enforces a single project-wide *grouping* style for the run of
    /// `use` statements at the top of a module body, chosen via `style`:
    /// - `single_group` — every `use` sits in one contiguous block with
    ///   no blank lines between imports.
    /// - `grouped` (default) — imports are partitioned into ordered
    ///   groups separated by exactly `blank_line_count` blank lines. The
    ///   default group set, in order, is std (`std` / `core` / `alloc`),
    ///   internal (`super` / `self` / `crate`), then third-party (every
    ///   other crate). The `order`, `std_crates`, `internal_prefixes`,
    ///   `cfg_block_handling`, and `blank_line_count` knobs tune the
    ///   partition; the inner ordering within each group is left to
    ///   `cargo fmt`.
    ///
    /// This rule only governs the *partitioning* of imports into blocks.
    /// Whether items within each `use` are merged or split is the job of
    /// `perfectionist::import_granularity`.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. Neither
    /// layout is wrong in the abstract — the violation is a mismatch
    /// with the project's configured `style`. Enforcing one keeps import
    /// blocks scanning uniformly and makes import diffs predictable.
    /// rustfmt's `group_imports` option can enforce the same shape, but
    /// only on the nightly channel; this lint gives stable-toolchain
    /// projects a hard CI check instead of a silent reformat.
    ///
    /// ### Example
    /// Under the default `style = "grouped"`:
    /// ```rust,ignore
    /// use clap::Parser;
    /// use std::time::Duration;
    /// use crate::args::Args;
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// use std::time::Duration;
    ///
    /// use crate::args::Args;
    ///
    /// use clap::Parser;
    /// ```
    pub perfectionist::IMPORT_GROUPING,
    Warn,
    "import grouping does not match the configured `import_grouping.style`",
    report_in_external_macro: false
}

/// Active by default. `grouped` is the shipped baseline; a project that
/// prefers one contiguous block sets `style = "single_group"` in
/// `dylint.toml`. Read by `register_pass`; gen-docs picks the constant
/// up to render the rule's default state.
pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Active;

const CONFIG_KEY: &str = "perfectionist::import_grouping";

pub struct ImportGrouping {
    config: Config,
}

impl ImportGrouping {
    fn new() -> Self {
        Self {
            config: dylint_linting::config_or_default(CONFIG_KEY),
        }
    }
}

impl_lint_pass!(ImportGrouping => [IMPORT_GROUPING]);

/// Register this rule's lint declaration. Paired with [`register_pass`];
/// see the module-level convention documented in `register_lints`.
pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[IMPORT_GROUPING]);
}

/// Install this rule's pass.
pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("import_grouping", DEFAULT_STATE) {
        return;
    }
    // Late pass: out-of-line `mod foo;` modules are `ModKind::Unloaded`
    // until macro expansion, so a pre-expansion pass never sees them.
    // `check_crate` re-parses each module file instead (see
    // [`crate::module_reparse`]), reaching every submodule while keeping
    // `#[cfg(...)]` gates intact — parsing does not strip cfg, the reason
    // a pre-expansion pass would otherwise be needed.
    lint_store.register_late_pass(|_| Box::new(ImportGrouping::new()));
}

/// One `use` statement admitted into a run. The submodules
/// ([`check`], [`render`]) read these fields directly.
pub(super) struct UseStmt<'ast> {
    item: &'ast Item,
    /// Group rank — see [`classify::rank`]. Smaller sorts earlier.
    pub(super) rank: usize,
    /// 1-based source line the statement (or its first attribute)
    /// starts on.
    pub(super) start_line: usize,
    /// 1-based source line the statement ends on (its `;`).
    pub(super) end_line: usize,
    /// Verbatim source text from the first attribute (or the `use`
    /// keyword) through the trailing `;`, reproduced unchanged.
    pub(super) text: String,
    /// Lowest byte position to replace — the start of the first
    /// attribute, or of the `use` keyword when there are none.
    lo: BytePos,
}

/// A detected violation parked until the enclosing HIR node is known.
/// Emission happens through [`span_lint_hir_and_then`] so a per-module /
/// per-item `#[allow]` / `#[expect]` resolves (see
/// [`crate::enclosing_hir`]).
struct Pending {
    /// Span used to resolve the lint-level anchor: the first `use` of the
    /// run, always contained by its own HIR node. Resolving on the run's
    /// replacement span instead would fall back to the crate root for an
    /// out-of-line `mod foo;`, whose item span lives in the parent file.
    anchor: Span,
    /// Span the diagnostic points at and rewrites — the whole run.
    span: Span,
    replacement: String,
    applicability: Applicability,
}

impl<'tcx> LateLintPass<'tcx> for ImportGrouping {
    fn check_crate(&mut self, lint_context: &LateContext<'tcx>) {
        let crates = parse_crate_module_files(lint_context);
        let mut violations: Vec<Pending> = Vec::new();
        for krate in &crates {
            self.check_items(lint_context, &krate.items, &mut violations);
        }
        if violations.is_empty() {
            return;
        }

        // Anchor each violation at its enclosing HIR node so a per-module
        // / per-item `#[allow]` resolves (emitting from `check_crate`
        // alone would sit at the crate root).
        let anchors: Vec<Span> = violations.iter().map(|pending| pending.anchor).collect();
        let hir_ids = find_enclosing_hir_ids(lint_context.tcx, &anchors);
        for (pending, hir_id) in violations.into_iter().zip(hir_ids) {
            let Pending {
                span,
                replacement,
                applicability,
                ..
            } = pending;
            span_lint_hir_and_then(
                lint_context,
                IMPORT_GROUPING,
                hir_id,
                span,
                self.message(),
                |diagnostic| {
                    diagnostic.span_suggestion(
                        span,
                        "regroup the imports",
                        replacement.clone(),
                        applicability,
                    );
                },
            );
        }
    }
}

impl ImportGrouping {
    fn check_items(
        &self,
        lint_context: &LateContext<'_>,
        items: &[Box<Item>],
        violations: &mut Vec<Pending>,
    ) {
        let mut run: Vec<UseStmt<'_>> = Vec::new();
        for item in items {
            match self.use_stmt(lint_context, item) {
                Some(stmt) => run.push(stmt),
                // A non-`use` item (including `extern crate`, kept above
                // the `use` block), a macro-expanded `use`, or one whose
                // source can't be recovered ends the current run.
                None => {
                    self.process_run(lint_context, &run, violations);
                    run.clear();
                }
            }
        }
        self.process_run(lint_context, &run, violations);

        // Descend into inline `mod { ... }` bodies. Out-of-line `mod foo;`
        // modules are `ModKind::Unloaded` in this fresh parse, but their
        // files appear in the source map in their own right and are
        // re-parsed by `check_crate`.
        for item in items {
            if let ItemKind::Mod(_, _, ModKind::Loaded(items, _, _)) = &item.kind {
                self.check_items(lint_context, items, violations);
            }
        }
    }

    fn use_stmt<'ast>(
        &self,
        lint_context: &LateContext<'_>,
        item: &'ast Item,
    ) -> Option<UseStmt<'ast>> {
        let ItemKind::Use(tree) = &item.kind else {
            return None;
        };
        if item.span.from_expansion() {
            return None;
        }
        // Only `#[cfg(...)]` gates the import's *existence*, which is what
        // the trailing cfg group is about. `#[cfg_attr(...)]` conditionally
        // applies some other attribute — the import itself is always
        // present — so it does not make a statement cfg-gated for grouping.
        let is_cfg_gated = item.attrs.iter().any(|attr| attr.has_name(sym::cfg));
        let rank = classify::rank(tree, is_cfg_gated, &self.config);

        // The replacement starts at the first attribute, or at the `use`
        // keyword when there are none. Attributes precede the item span, so
        // the minimum attribute `lo` is the lowest byte either way.
        let lo = item
            .attrs
            .iter()
            .map(|attr| attr.span.lo())
            .min()
            .unwrap_or(item.span.lo());
        let source_map = lint_context.sess().source_map();
        let text = source_map.span_to_snippet(item.span.with_lo(lo)).ok()?;
        let start_line = source_map.lookup_char_pos(lo).line;
        let end_line = source_map.lookup_char_pos(item.span.hi()).line;

        Some(UseStmt {
            item,
            rank,
            start_line,
            end_line,
            text,
            lo,
        })
    }

    fn process_run(
        &self,
        lint_context: &LateContext<'_>,
        run: &[UseStmt<'_>],
        violations: &mut Vec<Pending>,
    ) {
        // A run of one statement is a single group either way, so it
        // can never violate.
        if run.len() < 2 {
            return;
        }
        if check::is_compliant(self.config.style, self.config.blank_line_count, run) {
            return;
        }

        let first = &run[0];
        let last = &run[run.len() - 1];
        let replace_span = first
            .item
            .span
            .with_lo(first.lo)
            .with_hi(last.item.span.hi());
        let indent = indent_of(lint_context, first.item.span).unwrap_or(0);
        let pad = " ".repeat(indent);
        let replacement =
            render::replacement(self.config.style, self.config.blank_line_count, &pad, run);

        // A comment sitting between two statements is dropped by the
        // re-render (only each statement's own text is reproduced) and,
        // under `grouped`, may end up describing a statement that moved.
        // Down to `MaybeIncorrect` so the fix isn't applied unreviewed.
        let applicability = if self.run_has_interstatement_comment(lint_context, run) {
            Applicability::MaybeIncorrect
        } else {
            Applicability::MachineApplicable
        };

        violations.push(Pending {
            anchor: first.item.span,
            span: replace_span,
            replacement,
            applicability,
        });
    }

    /// Whether any gap between two source-adjacent statements in the run
    /// carries a comment.
    fn run_has_interstatement_comment(
        &self,
        lint_context: &LateContext<'_>,
        run: &[UseStmt<'_>],
    ) -> bool {
        let source_map = lint_context.sess().source_map();
        run.windows(2).any(|pair| {
            let gap = pair[0]
                .item
                .span
                .with_lo(pair[0].item.span.hi())
                .with_hi(pair[1].lo);
            source_map
                .span_to_snippet(gap)
                .is_ok_and(|snippet| snippet.contains("//") || snippet.contains("/*"))
        })
    }

    fn message(&self) -> &'static str {
        match self.config.style {
            Style::SingleGroup => {
                "blank lines split the imports; this project keeps them in a single group"
            }
            Style::Grouped => "imports are not partitioned into ordered groups",
        }
    }
}
