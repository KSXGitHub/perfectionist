use crate::common::DefaultState;
use crate::rule_index::{Register, rule};
use crate::test_code::fn_in_test_code;
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

mod config;
mod score;

use config::Config;
use score::{Score, score_body};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Measures the cognitive complexity of every function and method
    /// body and flags the ones above `max_complexity` (default `15`).
    ///
    /// The measure is SonarSource's Cognitive Complexity, applied to
    /// Rust: a count of how much control flow a reader has to hold in
    /// their head, weighted by how deeply it is nested. Each construct
    /// adds to the score as follows.
    ///
    /// | Construct                                                                         | Increment                                                    |
    /// |:---------------------------------------------------------------------------------:|:------------------------------------------------------------:|
    /// | `if`, `match`, `for`, `while`, `loop`                                             | 1, plus 1 for each enclosing `if` / `match` / loop / closure |
    /// | `else if`, `else`, `let ... else`, a match-arm guard                              | 1                                                            |
    /// | each run of like boolean operators (`a && b && c` is one; `a && b \|\| c` is two) | 1                                                            |
    /// | each labelled `break` or `continue`                                               | 1                                                            |
    /// | each call the function makes to itself                                            | 1                                                            |
    ///
    /// A closure adds nothing itself but deepens the nesting of what it
    /// contains. `?`, `.await`, `return`, and an unlabelled `break` or
    /// `continue` add nothing.
    ///
    /// The score counts what the author wrote. Code produced by a macro
    /// expansion contributes nothing, so a `println!` or a project's own
    /// `macro_rules!` is as cheap as a call, though an `if` written
    /// inside a macro's arguments still counts. A function that is
    /// itself produced by a macro is not measured. Nested functions are
    /// measured on their own, not as part of the function that contains
    /// them.
    ///
    /// Test code is measured like any other code; set
    /// `test_code_exception` to leave it alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A
    /// function that branches and loops many times over, at several
    /// levels of nesting, is doing more than one thing, and the reader
    /// has to reconstruct all of it to change any of it. The cap pushes
    /// such a function to be split into smaller ones, each named for
    /// the one thing it does, and it pushes deeply nested code to be
    /// flattened with early returns and `let ... else`. The metric is
    /// designed so that both moves lower the score: a `match` moved
    /// into its own function loses its nesting penalty, and a guard
    /// clause costs one where the `if` it replaces cost one per level.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::cognitive_complexity` (`restriction`, off by default) is
    /// a count of branches with a correction for early returns. It
    /// charges the same for a branch at the top of a function as for
    /// one five levels deep, charges every `?` and every desugared
    /// `while` condition as a branch, and measures the expansion of a
    /// macro rather than its invocation, so a function built from a
    /// local `macro_rules!` can score in the hundreds. This rule scores
    /// nesting, ignores what the reader does not see, and treats a
    /// macro invocation as one step. Enable one or the other, not both.
    ///
    /// `clippy::too_many_lines` and `clippy::excessive_nesting` measure
    /// length and depth on their own and pair well with this rule.
    ///
    /// ### Example
    ///
    /// **Avoid:** a nested `if` inside a `for` inside a `match` — the
    /// innermost `if` alone costs 3
    ///
    /// ```rust,ignore
    /// fn describe(kind: Kind, items: &[Item]) -> String {
    ///     match kind {
    ///         Kind::Listed => {
    ///             let mut out = String::new();
    ///             for item in items {
    ///                 if item.visible && !item.deprecated {
    ///                     out.push_str(&item.name);
    ///                 } else if item.deprecated {
    ///                     out.push_str("(deprecated)");
    ///                 }
    ///             }
    ///             out
    ///         }
    ///         Kind::Counted => items.len().to_string(),
    ///     }
    /// }
    /// ```
    ///
    /// **Prefer:** one function per thing, each flat
    ///
    /// ```rust,ignore
    /// fn describe(kind: Kind, items: &[Item]) -> String {
    ///     match kind {
    ///         Kind::Listed => list_names(items),
    ///         Kind::Counted => items.len().to_string(),
    ///     }
    /// }
    ///
    /// fn list_names(items: &[Item]) -> String {
    ///     items.iter().map(Item::label).collect()
    /// }
    ///
    /// impl Item {
    ///     fn label(&self) -> &str {
    ///         if self.deprecated {
    ///             return "(deprecated)";
    ///         }
    ///         if self.visible { &self.name } else { "" }
    ///     }
    /// }
    /// ```
    pub perfectionist::EXCESSIVE_COGNITIVE_COMPLEXITY,
    Warn,
    "function body has a cognitive complexity above the configured maximum",
    report_in_external_macro: false
}

pub struct ExcessiveCognitiveComplexity {
    config: Config,
}

impl_lint_pass!(ExcessiveCognitiveComplexity => [EXCESSIVE_COGNITIVE_COMPLEXITY]);

impl Register for rule::ExcessiveCognitiveComplexity {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[EXCESSIVE_COGNITIVE_COMPLEXITY]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(ExcessiveCognitiveComplexity {
                config: Config::load(),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for ExcessiveCognitiveComplexity {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _span: Span,
        def_id: LocalDefId,
    ) {
        // A closure is scored as part of the function that contains it.
        let (FnKind::ItemFn(ident, ..) | FnKind::Method(ident, ..)) = kind else {
            return;
        };
        let def_span = cx.tcx.def_span(def_id);
        if def_span.from_expansion() {
            return;
        }
        if self.config.test_code_exception && fn_in_test_code(cx, def_id) {
            return;
        }
        let score = score_body(cx, def_id, body);
        if score.total <= self.config.max_complexity {
            return;
        }
        emit(cx, def_span, ident.name, score, self.config.max_complexity);
    }
}

fn emit(cx: &LateContext<'_>, span: Span, name: rustc_span::Symbol, score: Score, max: usize) {
    let total = score.total;
    let from_nesting = score.from_nesting;
    let message = format!(
        "function `{name}` has a cognitive complexity of {total}, above the limit of {max}",
    );
    span_lint_and_then(cx, EXCESSIVE_COGNITIVE_COMPLEXITY, span, message, |diag| {
        if from_nesting > 0 {
            diag.note(format!(
                "nesting accounts for {from_nesting} of the {total}",
            ));
        }
        diag.help("split the function into smaller ones, each doing one thing, and flatten nested branches with early returns");
    });
}
