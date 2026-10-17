use crate::common::DefaultState;
use crate::defaulted_mapper::{DefaultedCall, defaulted_call};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Checks the usage of `map_or_else(_, _)` on `Option` and
    /// `Result`.
    ///
    /// Left alone: `map_or_default`, which takes no default; a
    /// receiver of any other type; and a call an expansion produced,
    /// though one written as a macro *argument* is flagged.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. The
    /// call names the default closure before the mapper it stands in
    /// for, so a reader meets the exceptional case first, and a diff
    /// shows the mapper — usually the part that changed — behind
    /// however many lines that closure takes.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::map_unwrap_or` (`pedantic`) is this rule's exact
    /// reverse, and its own documentation records the caveat this rule
    /// is founded on: the arguments are not in execution order.
    /// `clippy::option_if_let_else` (`nursery`) is the reverse of the
    /// `match` remedy. A project enabling either group has to allow
    /// that lint, or the two trade warnings at one call site.
    ///
    /// At its default levels Clippy reaches the method only where the
    /// mapper hands its parameter straight back
    /// (`clippy::unnecessary_option_map_or_else`,
    /// `clippy::unnecessary_result_map_or_else`). Every other call
    /// draws nothing, which is the gap this rule fills.
    ///
    /// ### Interaction with sibling rules
    ///
    /// `perfectionist::map_or` says the same of the sibling method.
    /// `perfectionist::splittable_adapter_closure` reads the same
    /// calls for a different fault — a mapper doing several jobs.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let label = parsed.map_or_else(|error| error.to_string(), Entry::into_name);
    /// ```
    ///
    /// **Prefer:** the chain, which reads in the order it runs
    ///
    /// ```rust,ignore
    /// let label = parsed.map(Entry::into_name).unwrap_or_else(|error| error.to_string());
    /// ```
    ///
    /// **Prefer:** a `match`, which names both cases
    ///
    /// ```rust,ignore
    /// let label = match parsed {
    ///     Ok(entry) => entry.into_name(),
    ///     Err(error) => error.to_string(),
    /// };
    /// ```
    pub perfectionist::MAP_OR_ELSE,
    Warn,
    "`map_or_else` call, which names its default before the mapper it stands in for",
    report_in_external_macro: false
}

/// The chain the method folds into one call.
const CHAIN_HELP: &str = "put the default last: `map(...).unwrap_or_else(...)`";

/// The remedy every call has, and the only one whose branches take more
/// than an expression.
const MATCH_HELP: &str = "or a `match`, which also takes arms that are more than an expression";

const CONFIG_KEY: &str = "perfectionist::map_or_else";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct MapOrElse;

impl_lint_pass!(MapOrElse => [MAP_OR_ELSE]);

impl Register for rule::MapOrElse {
    /// A method name and a receiver type decide the whole trigger, and
    /// every call they find is the shape the rule objects to, so there
    /// is no false-positive rate a baseline would have to hedge
    /// against.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[MAP_OR_ELSE]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(MapOrElse)));
    }
}

impl<'tcx> LateLintPass<'tcx> for MapOrElse {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let Some(call) = defaulted_call(cx, expr, "map_or_else") else {
            return;
        };
        emit(cx, &call);
    }
}

/// Report the call and name what it can become.
///
/// The default decides nothing here. Both remedies apply to every
/// `map_or_else`, and which one reads better is what the author can see
/// and the rule cannot.
fn emit(cx: &LateContext<'_>, call: &DefaultedCall<'_>) {
    span_lint_and_then(
        cx,
        MAP_OR_ELSE,
        call.span,
        "`map_or_else` names its default before the mapper it stands in for",
        |diag| {
            diag.help(CHAIN_HELP);
            diag.help(MATCH_HELP);
        },
    );
}
