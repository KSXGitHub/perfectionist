use crate::common::DefaultState;
use crate::defaulted_mapper::{DefaultedCall, Family, defaulted_call};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::{is_default_equivalent, is_none_expr};
use rustc_ast::LitKind;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Checks the usage of `map_or(_, _)` on `Option` and `Result`.
    ///
    /// Left alone: `map_or_default`, which takes no default; a
    /// receiver of any other type; and a call an expansion produced,
    /// though one written as a macro *argument* is flagged.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. The
    /// call names the default before the mapper it stands in for, so
    /// it states the exceptional case ahead of the ordinary one, and a
    /// diff shows the mapper — usually the part that changed — behind
    /// it.
    ///
    /// The help names what states the two the other way round: a
    /// `match`, which fits any call and is the only remedy that leaves
    /// the default unevaluated where there is a value; the chain
    /// `map(_).unwrap_or(_)`; and, for some defaults, one shorter call
    /// — `false` into `is_some_and` (`is_ok_and` on a `Result`), `true`
    /// into `is_none_or`, `None` into `and_then`, and a default the
    /// type's `Default` gives into `map(_).unwrap_or_default()`. Those
    /// predicates need Rust 1.70, or 1.82 for `is_none_or`, so the help
    /// names the chain beside them.
    /// Which remedy reads best is the author's, so the rule suggests
    /// none of them.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::map_unwrap_or` (`pedantic`) is this rule's exact
    /// reverse, and its own documentation records the caveat this rule
    /// is founded on: the arguments are not in execution order.
    /// `clippy::option_if_let_else` (`nursery`) is the reverse of the
    /// `match` remedy. A project enabling either group has to allow
    /// that lint for these remedies to stand.
    ///
    /// At its default levels Clippy redirects a boolean default
    /// (`clippy::unnecessary_map_or`, itself gated on the crate's
    /// `rust-version`) and a `None` one (`clippy::option_map_or_none`,
    /// `clippy::result_map_or_into_option`). Every other default draws
    /// nothing, which is the gap this rule fills.
    ///
    /// ### Interaction with sibling rules
    ///
    /// `perfectionist::map_or_else_usage` holds `map_or_else` to the
    /// same order. `perfectionist::splittable_adapter_closure` reads
    /// the same calls for a different fault — a mapper doing several
    /// jobs.
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
    pub perfectionist::MAP_OR_USAGE,
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

const CONFIG_KEY: &str = "perfectionist::map_or_usage";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct MapOrUsage;

impl_lint_pass!(MapOrUsage => [MAP_OR_USAGE]);

impl Register for rule::MapOrUsage {
    /// A method name and a receiver type decide the whole trigger, and
    /// every call they find is the shape the rule objects to, so there
    /// is no false-positive rate a baseline would have to hedge
    /// against.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[MAP_OR_USAGE]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(MapOrUsage)));
    }
}

impl<'tcx> LateLintPass<'tcx> for MapOrUsage {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let Some(call) = defaulted_call(cx, expr, "map_or") else {
            return;
        };
        emit(cx, &call);
    }
}

/// What the help names, which the default decides.
enum Remedy {
    /// One method of the receiver's own, answering what the whole call
    /// answers. `since` is the Rust version that method arrived in,
    /// where a crate may predate it.
    OneCall {
        method: &'static str,
        since: Option<&'static str>,
    },
    /// The chain, ending in `unwrap_or_default` because the default is
    /// one the type's own `Default` gives.
    DefaultedChain,
    /// The chain. Nothing about this default is shorter than it.
    Chain,
}

/// The remedy `call`'s default earns.
///
/// A boolean answers a test each family already has a name for, and a
/// `None` answers the `and_then` an `Option` already has. `Result` is
/// missing the counterpart of `is_none_or`, so a `true` default
/// collapses into nothing there; a `None` default on a `Result` is left
/// to the chain as well, since reaching `and_then` would mean an `ok()`
/// first, which is a step rather than a shorter spelling.
fn remedy(cx: &LateContext<'_>, call: &DefaultedCall<'_>) -> Remedy {
    let one_call = |method, since| Remedy::OneCall { method, since };
    if let ExprKind::Lit(literal) = call.default.kind
        && let LitKind::Bool(value) = literal.node
    {
        return match (call.family, value) {
            (Family::Option, false) => one_call("is_some_and", Some("1.70")),
            (Family::Option, true) => one_call("is_none_or", Some("1.82")),
            (Family::Result, false) => one_call("is_ok_and", Some("1.70")),
            (Family::Result, true) => Remedy::Chain,
        };
    }
    if call.family == Family::Option && is_none_expr(cx, call.default) {
        return one_call("and_then", None);
    }
    // A default the type's own `Default` gives, written as the call that
    // gives it. A literal `0` or `""` is left to the chain: it names the
    // value it stands for where `unwrap_or_default` hides it, and
    // `clippy::unwrap_or_default` draws the line in the same place.
    if matches!(call.default.kind, ExprKind::Call(..)) && is_default_equivalent(cx, call.default) {
        return Remedy::DefaultedChain;
    }
    Remedy::Chain
}

/// Report the call and name what it can become.
fn emit(cx: &LateContext<'_>, call: &DefaultedCall<'_>) {
    span_lint_and_then(
        cx,
        MAP_OR_USAGE,
        call.span,
        "`map_or` names its default before the mapper it stands in for",
        |diag| {
            match remedy(cx, call) {
                // The chain rides along where the named method is
                // younger than `map_or` itself, so a crate pinned below
                // that version still reads advice it can take.
                Remedy::OneCall {
                    method,
                    since: Some(version),
                } => {
                    diag.help(format!(
                        "that default is what `{method}` means (Rust {version} or later)",
                    ));
                    diag.help(CHAIN_HELP);
                }
                Remedy::OneCall {
                    method,
                    since: None,
                } => {
                    diag.help(format!("that default is what `{method}` means"));
                }
                Remedy::DefaultedChain => {
                    diag.help("the default is the type's own: `map(...).unwrap_or_default()`");
                }
                Remedy::Chain => {
                    diag.help(CHAIN_HELP);
                }
            }
            // Every default has this one, so every diagnostic ends on
            // it rather than reading as though the call above were the
            // answer.
            diag.help(MATCH_HELP);
        },
    );
}
