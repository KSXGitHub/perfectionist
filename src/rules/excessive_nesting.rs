use crate::common::DefaultState;
use crate::measured_fn::measured_fn;
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

mod depth;

use depth::deepest_nesting;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Measures how deeply the constructs in a function or method body
    /// nest and flags the body when the deepest point is more than
    /// `max_depth` (default `3`) levels down.
    ///
    /// A construct is one level: an `if` (an `else if` stays at the
    /// same level; an `else` body is inside), a `match` (its arms are
    /// inside), a `for`, `while`, or `loop`, a closure, the body of a
    /// `let ... else`, and a free-standing block such as an `unsafe`
    /// block or the block a `let` initialises from. The block that is a
    /// construct's own body is not a level of its own, so
    /// `if ready { work() }` is one level, not two.
    ///
    /// The depth counts what the author wrote. A construct produced by
    /// a macro expansion adds no level, though an `if` written inside a
    /// macro's arguments still counts; `?`, `.await`, and the desugared
    /// shape of `for`, `while`, and `async` add nothing. A function
    /// produced by a macro is not measured, and a nested function is
    /// measured on its own.
    ///
    /// Test code is measured like any other code; set
    /// `test_code_exception` to leave it alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Each
    /// level of nesting is a condition the reader has to keep true in
    /// their head while reading everything inside it; past three, the
    /// code at the deepest point can only be understood by re-reading
    /// the way in. Deep nesting almost always flattens: a guard clause
    /// or `let ... else` returns early instead of wrapping the rest, an
    /// inner loop body becomes a function, an arm's body becomes a
    /// call. The limit of three is the one SonarSource ships.
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
    /// **Avoid:** four levels — `for`, `if let`, `match`, `if`
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
    /// **Prefer:** a guard and a function per level of meaning
    ///
    /// ```rust,ignore
    /// for entry in entries {
    ///     let Some(meta) = entry.metadata() else { continue };
    ///     visit(entry, meta, limit);
    /// }
    ///
    /// fn visit(entry: Entry, meta: Meta, limit: u64) {
    ///     match meta.kind() {
    ///         Kind::File if meta.len() > limit => report(entry),
    ///         Kind::File => {}
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
    test_code_exception: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            test_code_exception: false,
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
        let Some(function) = measured_fn(cx, kind, def_id, self.config.test_code_exception) else {
            return;
        };
        let Some(deepest) = deepest_nesting(cx.tcx, body) else {
            return;
        };
        if deepest.depth <= self.config.max_depth {
            return;
        }
        let max = self.config.max_depth;
        let name = function.name;
        let depth = deepest.depth;
        let noun = if depth == 1 { "level" } else { "levels" };
        let message =
            format!("function `{name}` nests {depth} {noun} deep, above the limit of {max}");
        let deepest_span = cx.sess().source_map().span_until_whitespace(deepest.span);
        span_lint_and_then(cx, EXCESSIVE_NESTING, function.span, message, |diag| {
            diag.span_note(deepest_span, format!("this is {depth} {noun} deep"));
            diag.help("return early with a guard clause or `let ... else`, or move the inner levels into their own function");
        });
    }
}
