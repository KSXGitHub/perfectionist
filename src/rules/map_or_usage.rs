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
    /// Flags a call to `Option::map_or` or `Result::map_or`, which take
    /// the default first and the mapper second, and names the remedies
    /// that put them back in the order they run.
    ///
    /// Which remedy the help names depends on the default:
    ///
    /// | The call                      | Prefer                         |
    /// |-------------------------------|--------------------------------|
    /// | `opt.map_or(false, f)`        | `opt.is_some_and(f)`           |
    /// | `opt.map_or(true, f)`         | `opt.is_none_or(f)`            |
    /// | `res.map_or(false, f)`        | `res.is_ok_and(f)`             |
    /// | `opt.map_or(None, f)`         | `opt.and_then(f)`              |
    /// | `x.map_or(String::new(), f)`  | `x.map(f).unwrap_or_default()` |
    /// | `x.map_or(default, f)`        | `x.map(f).unwrap_or(default)`  |
    ///
    /// The predicates are younger than `map_or` itself — `is_some_and`
    /// and `is_ok_and` stabilised in Rust 1.70 and `is_none_or` in
    /// 1.82 — so the help names the chain beside each one, which every
    /// version has. `Result` has no counterpart of `is_none_or`, so
    /// `res.map_or(true, f)` takes the chain as well.
    ///
    /// Where the default collapses into no shorter call, the help names
    /// the chain and a `match`. The `match` is the remedy when an arm
    /// would hold more than an expression, and the only one that leaves
    /// the default out of the path that does not need it, since
    /// `map_or` and `unwrap_or` both evaluate it either way.
    ///
    /// Left alone:
    ///
    /// - `map_or_default`, which takes the mapper and nothing else, so
    ///   it has no order to get wrong.
    /// - A receiver of any other type, however its own `map_or` is
    ///   shaped.
    /// - A call an expansion produced, whose line its reader cannot act
    ///   on. A call written as a macro *argument* is the author's own
    ///   and is flagged.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. The
    /// call returns exactly what each remedy returns.
    ///
    /// The preference is that a call should read in the order it runs.
    /// A reader who reaches `width.map_or(` is handed a receiver and
    /// must then hold a number that means nothing yet, until the mapper
    /// arrives to explain it. The chain hands over the receiver, then
    /// what happens to it, then what stands in when there is nothing to
    /// happen to — the order the machine takes them in. A diff is where
    /// that costs most: the mapper is usually the part that changed, and
    /// it sits behind an argument belonging to the other branch, so a
    /// reviewer reads the exception first and the change second.
    ///
    /// What the preference gives up is one call's worth of brevity.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::map_unwrap_or` is this rule's exact reverse: it rewrites
    /// `map(f).unwrap_or(a)` into `map_or(a, f)`. It is in the
    /// `pedantic` group, so allow-by-default, and its own documentation
    /// records the same observation this rule is founded on — that the
    /// arguments are not in execution order. A project that enables
    /// `clippy::pedantic` has to allow that lint for this one's remedies
    /// to stand. `clippy::option_if_let_else`, in the `nursery` group,
    /// is the reverse of the `match` remedy and takes the same
    /// treatment.
    ///
    /// Three warn-by-default lints already push particular defaults off
    /// the method and agree with this rule's direction:
    /// `clippy::unnecessary_map_or` sends a boolean default to
    /// `is_some_and` or `is_none_or`, `clippy::option_map_or_none` sends
    /// a `None` default to `and_then`, and
    /// `clippy::result_map_or_into_option` sends `map_or(None, Some)` to
    /// `ok()`. The first of them is gated on the crate's `rust-version`,
    /// so a crate pinned below 1.82 hears nothing from it; every other
    /// default draws nothing from Clippy at its default levels, which is
    /// the gap this rule fills.
    ///
    /// ### Interaction with sibling rules
    ///
    /// `perfectionist::map_or_else_usage` holds `map_or_else` to the
    /// same order. The two are separate rules because the remedies are:
    /// a default that is a value has predicates to collapse into, where
    /// a default that is a closure has only the chain and the `match`.
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
    /// let width = terminal_width.map_or(80, |width| width.saturating_sub(2));
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// let width = terminal_width.map(|width| width.saturating_sub(2)).unwrap_or(80);
    /// ```
    pub perfectionist::MAP_OR_USAGE,
    Warn,
    "`map_or` call, whose default stands before the mapper, in the reverse of the order they run",
    report_in_external_macro: false
}

/// The remedy that always applies, whatever the default is: the chain
/// the method folds into one call.
const CHAIN_HELP: &str = "put the default last: `map(...).unwrap_or(...)` states the steps in the \
                          order they run";

/// The remedy for a default that is more than a value to hand over, and
/// the only one that stops the default being evaluated where the
/// receiver holds a value: `map_or` and `unwrap_or` both evaluate it
/// either way.
const MATCH_HELP: &str = "or a `match`, which names both cases and leaves the default out of the \
                          path that does not need it";

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
    /// The chain, or a `match`. Nothing about this default is shorter
    /// than either.
    ChainOrMatch,
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
            (Family::Result, true) => Remedy::ChainOrMatch,
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
    Remedy::ChainOrMatch
}

/// Report the call and name what it can become.
fn emit(cx: &LateContext<'_>, call: &DefaultedCall<'_>) {
    span_lint_and_then(
        cx,
        MAP_OR_USAGE,
        call.span,
        "`map_or` takes the default before the mapper, in the reverse of the order they run",
        |diag| match remedy(cx, call) {
            // The chain rides along where the named method is younger
            // than `map_or` itself, so a crate pinned below that
            // version still reads advice it can take.
            Remedy::OneCall {
                method,
                since: Some(version),
            } => {
                diag.help(format!(
                    "that default is what `{method}` means: call it instead, where the crate is \
                     on Rust {version} or later",
                ));
                diag.help(CHAIN_HELP);
            }
            Remedy::OneCall {
                method,
                since: None,
            } => {
                diag.help(format!(
                    "that default is what `{method}` means: call it instead",
                ));
            }
            Remedy::DefaultedChain => {
                diag.help(
                    "the default is one the type's own `Default` gives: \
                     `map(...).unwrap_or_default()`",
                );
            }
            Remedy::ChainOrMatch => {
                diag.help(CHAIN_HELP);
                diag.help(MATCH_HELP);
            }
        },
    );
}
