use crate::common::DefaultState;
use crate::measured_fn::measured_fn;
use crate::rule_index::{Register, rule};
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

mod depth;
mod emit;

use depth::deepest_nesting;
use emit::emit;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Measures how deeply the constructs in a function or method body
    /// nest and flags the body when the deepest point is more than
    /// `max_depth` (default `3`) levels down.
    ///
    /// Each construct is a level: an `if` (an `else if` stays at the
    /// same level; an `else` body is inside), a `match` (its arms are
    /// inside), a `for`, `while`, or `loop`, a closure, the body of a
    /// `let ... else`, and a free-standing block such as an `unsafe`
    /// block or the block a `let` initialises from. The block that is a
    /// construct's own body is not a level of its own, so
    /// `if ready { work() }` is 1 level, not 2.
    ///
    /// The depth counts what the author wrote. A construct produced by
    /// a macro expansion is not a level, though an `if` written inside a
    /// macro's arguments still counts; `?` and `.await` are not levels,
    /// a `for` or `while` loop counts once, and the body of an
    /// `async fn` or `async` block is not a level of its own. A function produced
    /// by a macro is not measured, and a nested function is measured on
    /// its own.
    ///
    /// Test code is measured like any other code; set
    /// `exempt_tests` to leave it alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Each
    /// level of nesting is a condition the reader has to keep true in
    /// their head while reading everything inside it; past three, the
    /// code at the deepest point can only be understood by re-reading
    /// the way in. Deep nesting almost always flattens: a guard clause
    /// or `let ... else` returns early instead of wrapping the rest, an
    /// arm guard folds a condition into a pattern, `continue` unindents
    /// a loop body. Extracting the inner levels answers it just as
    /// well, when the new function can be named for what it does rather
    /// than where it came from and needs few of the enclosing locals; one
    /// that takes most of them as parameters has moved the nesting into
    /// an argument list rather than removed it. The limit of three is the
    /// one SonarSource ships.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::excessive_nesting` (`complexity`, but inert until
    /// `excessive-nesting-threshold` is set) counts every brace pair
    /// from the crate root, `mod`, `impl`, and `fn` included, so a
    /// method in an `impl` in a module already sits at three before
    /// its first `if`, and one threshold has to serve files of every
    /// shape. This rule measures each function body on its own, from
    /// zero, and counts constructs rather than braces. Enable one or the
    /// other, not both.
    ///
    /// ### Example
    ///
    /// **Avoid:** 4 levels — `for`, `if let`, `match`, `if`
    ///
    /// ```rust,ignore
    /// for entry in entries {
    ///     if let Some(meta) = entry.metadata() {
    ///         match meta.kind() {
    ///             Kind::File => {
    ///                 if meta.len() > limit {
    ///                     report(entry);
    ///                 }
    ///             }
    ///             Kind::Dir => descend(entry),
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// **Prefer:** 2 levels — a `let ... else` guard in place of the
    /// `if let`, an arm guard in place of the `if`
    ///
    /// ```rust,ignore
    /// for entry in entries {
    ///     let Some(meta) = entry.metadata() else { continue };
    ///     match meta.kind() {
    ///         Kind::File if meta.len() > limit => report(entry),
    ///         Kind::File => {}
    ///         Kind::Dir => descend(entry),
    ///     }
    /// }
    /// ```
    ///
    /// **Prefer:** the same body, cut where it has a name of its own —
    /// `report_or_descend` is named for what it does and takes three
    /// values, none of them the loop's state. That leaves 2 levels on
    /// each side of the cut.
    ///
    /// ```rust,ignore
    /// for entry in entries {
    ///     if let Some(meta) = entry.metadata() {
    ///         report_or_descend(entry, meta, limit);
    ///     }
    /// }
    ///
    /// fn report_or_descend(entry: Entry, meta: Meta, limit: u64) {
    ///     match meta.kind() {
    ///         Kind::File => {
    ///             if meta.len() > limit {
    ///                 report(entry);
    ///             }
    ///         }
    ///         Kind::Dir => descend(entry),
    ///     }
    /// }
    /// ```
    pub perfectionist::EXCESSIVE_NESTING,
    Warn,
    "function body nests constructs deeper than the configured maximum",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::excessive_nesting";

/// The depth SonarSource's "control flow statements should not be
/// nested too deeply" rule ships with.
const DEFAULT_MAX_DEPTH: usize = 3;

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// The deepest a construct may sit without the function being
    /// flagged. Defaults to `3`.
    max_depth: usize,
    /// Whether test code is left alone: functions inside a
    /// `#[cfg(test)]` module, `#[test]` functions, and everything in
    /// an integration-test or benchmark target. Defaults to `false`,
    /// so a test is held to the same limit as the code it exercises.
    exempt_tests: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            exempt_tests: false,
        }
    }
}

pub struct ExcessiveNesting {
    config: Config,
}

impl_lint_pass!(ExcessiveNesting => [EXCESSIVE_NESTING]);

impl Register for rule::ExcessiveNesting {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[EXCESSIVE_NESTING]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(ExcessiveNesting {
                config: dylint_linting::config_or_default(CONFIG_KEY),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for ExcessiveNesting {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _span: Span,
        def_id: LocalDefId,
    ) {
        let Some(function) = measured_fn(cx, kind, def_id, self.config.exempt_tests) else {
            return;
        };
        let Some(deepest) = deepest_nesting(cx.tcx, body) else {
            return;
        };
        if deepest.depth() <= self.config.max_depth {
            return;
        }
        emit(cx, &function, &deepest, self.config.max_depth);
    }
}
