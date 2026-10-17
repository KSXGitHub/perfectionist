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
    /// Flags a call to `Option::map_or_else` or `Result::map_or_else`,
    /// which take the default closure first and the mapper second, and
    /// names the remedies that put them back in the order they run:
    /// the chain `map(...).unwrap_or_else(...)`, or a `match`.
    ///
    /// The chain carries both closures over unchanged, the error one
    /// included: `Result::map_or_else` hands its first closure the
    /// error, and so does `Result::unwrap_or_else`. A `match` is the
    /// remedy where an arm would hold more than an expression —
    /// several error variants to tell apart, a binding to name, a `?`
    /// or a `return` to reach for.
    ///
    /// Left alone:
    ///
    /// - `map_or_default`, which takes the mapper and nothing else, so
    ///   it has no order to get wrong.
    /// - A receiver of any other type, however its own `map_or_else`
    ///   is shaped.
    /// - A call an expansion produced, whose line its reader cannot
    ///   act on. A call written as a macro *argument* is the author's
    ///   own and is flagged.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. The
    /// call returns exactly what each remedy returns, and since only
    /// one of the two closures ever runs, moving them changes neither
    /// what runs nor when.
    ///
    /// The preference is that a call should read in the order it runs.
    /// The default closure is a body rather than a token, so the
    /// mapper — the part a reader is usually looking for, and the part
    /// a diff usually changed — ends up behind however many lines the
    /// other branch takes. The chain puts the receiver first, then
    /// what happens to it, then what stands in when there is nothing
    /// to happen to, which is the order the machine takes them in.
    ///
    /// What the preference gives up is one call's worth of brevity.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::map_unwrap_or` is this rule's exact reverse: it
    /// rewrites `map(f).unwrap_or_else(g)` into `map_or_else(g, f)`.
    /// It is in the `pedantic` group, so allow-by-default, and its own
    /// documentation records the same observation this rule is founded
    /// on — that the arguments are not in execution order. A project
    /// that enables `clippy::pedantic` has to allow that lint for this
    /// one's remedies to stand. `clippy::option_if_let_else`, in the
    /// `nursery` group, is the reverse of the `match` remedy and takes
    /// the same treatment.
    ///
    /// At its default levels Clippy says nothing about `map_or_else`
    /// beyond the degenerate case the
    /// `clippy::unnecessary_option_map_or_else` and
    /// `clippy::unnecessary_result_map_or_else` pair covers, where the
    /// mapper hands its parameter straight back. Where the default
    /// closure is one the type's own `Default` gives, prefer
    /// `unwrap_or_default()` to close the chain, which is what
    /// `clippy::unwrap_or_default` asks for in turn.
    ///
    /// ### Interaction with sibling rules
    ///
    /// `perfectionist::map_or_usage` holds `map_or` to the same order.
    /// The two are separate rules because the remedies are: a default
    /// that is a value has predicates to collapse into — `is_some_and`
    /// and the rest — where a default that is a closure has only the
    /// chain and the `match`.
    ///
    /// `perfectionist::splittable_adapter_closure` reads the same calls
    /// for a different fault — a mapper doing several jobs — and asks
    /// for the closure to be split rather than the call to go. A call
    /// that draws both diagnostics can take either fix first.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let label = parsed.map_or_else(|error| error.to_string(), Entry::into_name);
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let label = parsed.map(Entry::into_name).unwrap_or_else(|error| error.to_string());
    /// ```
    pub perfectionist::MAP_OR_ELSE_USAGE,
    Warn,
    "`map_or_else` call, whose default stands before the mapper, in the reverse of the order they run",
    report_in_external_macro: false
}

/// The chain the method folds into one call, and the shorter ending a
/// default-producing closure earns there.
const CHAIN_HELP: &str = "put the default last: `map(...).unwrap_or_else(...)` states the steps in \
                          the order they run, and `map(...).unwrap_or_default()` says it shorter \
                          where the default is the one the type's own `Default` gives";

/// The remedy for a default whose body outgrows a closure.
const MATCH_HELP: &str = "or a `match`, which names both cases and keeps the mapper next to the \
                          value it maps";

const CONFIG_KEY: &str = "perfectionist::map_or_else_usage";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct MapOrElseUsage;

impl_lint_pass!(MapOrElseUsage => [MAP_OR_ELSE_USAGE]);

impl Register for rule::MapOrElseUsage {
    /// A method name and a receiver type decide the whole trigger, and
    /// every call they find is the shape the rule objects to, so there
    /// is no false-positive rate a baseline would have to hedge
    /// against.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[MAP_OR_ELSE_USAGE]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(MapOrElseUsage)));
    }
}

impl<'tcx> LateLintPass<'tcx> for MapOrElseUsage {
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
        MAP_OR_ELSE_USAGE,
        call.span,
        "`map_or_else` takes the default before the mapper, in the reverse of the order they run",
        |diag| {
            diag.help(CHAIN_HELP);
            diag.help(MATCH_HELP);
        },
    );
}
