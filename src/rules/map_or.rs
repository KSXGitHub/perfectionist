use crate::common::DefaultState;
use crate::defaulted_mapper::{Collapse, DefaultedCall, MAP_OR_DEFAULT, collapse, defaulted_call};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::{span_lint_and_sugg, span_lint_and_then};
use clippy_utils::msrvs::Msrv;
use clippy_utils::source::snippet_with_applicability;
use rustc_errors::Applicability;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Checks the usage of `map_or(_, _)` on `Option` and `Result`.
    ///
    /// Where the default is the one the mapped type's `Default` gives,
    /// the call is rewritten as `map_or_default`, which a fixer run
    /// applies. A `Foo::new()` qualifies only where `Foo::default()`
    /// is that very call, and a crate whose `rust-version` predates
    /// `map_or_default` keeps the help text instead. Any other default
    /// is the author's to settle.
    ///
    /// Left alone: `map_or_default`, which takes no default; a
    /// receiver of any other type; and a call an expansion produced,
    /// though one written as a macro *argument* is flagged.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. The
    /// call names the default before the mapper it stands in for, so a
    /// reader meets the exceptional case first, and a diff shows the
    /// mapper — usually the part that changed — behind it.
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
    /// At its default levels Clippy redirects a boolean default
    /// (`clippy::unnecessary_map_or`, itself gated on the crate's
    /// `rust-version`) and a `None` one (`clippy::option_map_or_none`,
    /// `clippy::result_map_or_into_option`). Every other default draws
    /// nothing, which is the gap this rule fills.
    ///
    /// ### Interaction with sibling rules
    ///
    /// `perfectionist::map_or_else` says the same of the sibling
    /// method. `perfectionist::splittable_adapter_closure` reads the
    /// same calls for a different fault — a mapper doing several jobs.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let width = terminal_width.map_or(80, |width| width.saturating_sub(2));
    /// ```
    ///
    /// **Prefer:** the chain, which reads in the order it runs
    ///
    /// ```rust,ignore
    /// let width = terminal_width.map(|width| width.saturating_sub(2)).unwrap_or(80);
    /// ```
    ///
    /// **Prefer:** a `match`, which names both cases
    ///
    /// ```rust,ignore
    /// let width = match terminal_width {
    ///     Some(width) => width.saturating_sub(2),
    ///     None => 80,
    /// };
    /// ```
    pub perfectionist::MAP_OR,
    Warn,
    "`map_or` call, which names its default before the mapper it stands in for",
    report_in_external_macro: false
}

/// The chain the method folds into one call, which every default can
/// take.
const CHAIN_HELP: &str = "put the default last: `map(...).unwrap_or(...)`";

/// The remedy every call has, whatever its default, and the only one
/// that stops the default being evaluated where the receiver holds a
/// value: `map_or` and `unwrap_or` both evaluate it either way.
const MATCH_HELP: &str =
    "or a `match`, which also leaves the default unevaluated where there is a value";

/// What the diagnostic says, whichever remedy follows it.
const MESSAGE: &str = "`map_or` names its default before the mapper it stands in for";

const CONFIG_KEY: &str = "perfectionist::map_or";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

#[derive(Default)]
pub struct MapOr {
    /// The crate's `rust-version`, which says whether the
    /// `map_or_default` the fix writes would compile there.
    msrv: Msrv,
}

impl_lint_pass!(MapOr => [MAP_OR]);

impl Register for rule::MapOr {
    /// A method name and a receiver type decide the whole trigger, and
    /// every call they find is the shape the rule objects to, so there
    /// is no false-positive rate a baseline would have to hedge
    /// against.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[MAP_OR]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(MapOr::default())));
    }
}

impl<'tcx> LateLintPass<'tcx> for MapOr {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        self.msrv.read_cargo(cx.sess());
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let Some(call) = defaulted_call(cx, expr, "map_or") else {
            return;
        };
        emit(cx, &call, self.msrv);
    }
}

/// Report the call and name what it can become.
///
/// The one remedy the rule applies itself is `map_or_default`, where
/// the default is the mapped type's own: it answers what the call
/// answers, in one call, so neither the chain nor a `match` is worth
/// naming beside it. Every other default leaves the choice open, and
/// the help names the candidates.
fn emit<'tcx>(cx: &LateContext<'tcx>, call: &DefaultedCall<'tcx>, msrv: Msrv) {
    let collapse = collapse(cx, call, Some(call.default));
    if let Some(Collapse::Default) = collapse
        && msrv.meets(cx, MAP_OR_DEFAULT)
    {
        let mut applicability = Applicability::MachineApplicable;
        let mapper = snippet_with_applicability(cx, call.mapper.span, "..", &mut applicability);
        span_lint_and_sugg(
            cx,
            MAP_OR,
            call.span,
            MESSAGE,
            "the default is the mapped type's own, so one call says it",
            format!("map_or_default({mapper})"),
            applicability,
        );
        return;
    }
    span_lint_and_then(cx, MAP_OR, call.span, MESSAGE, |diag| {
        match collapse {
            // The chain rides along where the named predicate is
            // younger than `map_or` itself, so a crate pinned below
            // that version still reads advice it can take.
            Some(Collapse::Predicate { method, since }) => {
                diag.help(format!(
                    "that default is what `{method}` means (Rust {since} or later)",
                ));
                diag.help(CHAIN_HELP);
            }
            Some(Collapse::AndThen) => {
                diag.help("that default is what `and_then` means");
            }
            // Reached only where the crate predates
            // `map_or_default`, since the fix above takes this
            // default otherwise.
            Some(Collapse::Default) => {
                diag.help("the default is the mapped type's own: `map(...).unwrap_or_default()`");
            }
            None => {
                diag.help(CHAIN_HELP);
            }
        }
        // Every default has this one, so every diagnostic ends on
        // it rather than reading as though the call above were the
        // answer.
        diag.help(MATCH_HELP);
    });
}
