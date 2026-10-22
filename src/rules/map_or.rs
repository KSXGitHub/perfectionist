use crate::common::DefaultState;
use crate::defaulted_mapper::{
    Collapse, DefaultedCall, MAP_OR_DEFAULT, collapse, default_is_simple, defaulted_call,
};
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
    /// A default that is a literal or a path is left alone, since
    /// meeting such a default before the mapper costs the reader
    /// nothing. `exempt_simple_default` turns that exemption off. A
    /// closure is not a simple default, and neither is an expression
    /// written as a macro call.
    ///
    /// Left alone whatever the configuration: `map_or_default`,
    /// which takes no default; a receiver of any other type; and a
    /// call an expansion produced, though one written as a macro
    /// *argument* is flagged.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. The
    /// call names the default before the mapper it stands in for, so a
    /// reader meets the exceptional case first. The mapper is usually
    /// the part that changed, and a diff shows it behind the default.
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
    /// same calls for a different fault: a mapper doing several jobs.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// let layout = terminal_width.map_or(
    ///     Layout {
    ///         columns: 80,
    ///         wrap: true,
    ///     },
    ///     Layout::fitting,
    /// );
    /// ```
    ///
    /// **Prefer:** the chain, which reads in the order it runs
    ///
    /// ```rust,ignore
    /// let layout = terminal_width.map(Layout::fitting).unwrap_or(Layout {
    ///     columns: 80,
    ///     wrap: true,
    /// });
    /// ```
    ///
    /// **Prefer:** a `match`, which names both cases
    ///
    /// ```rust,ignore
    /// let layout = match terminal_width {
    ///     Some(width) => Layout::fitting(width),
    ///     None => Layout {
    ///         columns: 80,
    ///         wrap: true,
    ///     },
    /// };
    /// ```
    pub perfectionist::MAP_OR,
    Warn,
    "`map_or` call, which names its default before the mapper it stands in for",
    report_in_external_macro: false
}

/// The chain the method folds into one call.
const CHAIN_HELP: &str = "put the default last: `map(...).unwrap_or(...)`";

/// The remedy every call has, whatever its default, and the only one
/// that leaves the default unevaluated where there is a value:
/// `map_or` and `unwrap_or` both evaluate it either way.
const MATCH_HELP: &str =
    "or a `match`, which also leaves the default unevaluated where there is a value";

/// What the diagnostic says, whichever remedy follows it.
const MESSAGE: &str = "`map_or` names its default before the mapper it stands in for";

const CONFIG_KEY: &str = "perfectionist::map_or";

/// The rule's configuration.
#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Whether a call whose default is a literal or a path
    /// (`option.map_or(0, mapper)`) is exempt, since meeting such a
    /// default before the mapper costs the reader nothing. A negated
    /// numeric literal counts as a literal. A closure does not count,
    /// however short, and neither does an expression written as a
    /// macro call, a struct or tuple literal, or an enum variant
    /// written as either, though a unit variant is a path. Defaults to
    /// `true`; set `false` to flag those calls too.
    exempt_simple_default: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            exempt_simple_default: true,
        }
    }
}

pub struct MapOr {
    /// The crate's `rust-version`, which says whether the
    /// `map_or_default` the fix writes would compile there.
    msrv: Msrv,
    /// Whether a simple default is left alone, from
    /// [`Config::exempt_simple_default`].
    exempt_simple_default: bool,
}

impl_lint_pass!(MapOr => [MAP_OR]);

impl Register for rule::MapOr {
    /// The trigger is a method name, a receiver type and a default the
    /// reader has to hold on to, and every call meeting all three is
    /// the shape the rule objects to, so there is no false-positive
    /// rate a baseline would have to hedge against.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[MAP_OR]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let exempt_simple_default = config.exempt_simple_default;
        lint_store.register_late_lint_pass(Box::new(move |_| {
            Box::new(MapOr {
                msrv: Msrv::default(),
                exempt_simple_default,
            })
        }));
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
        if self.exempt_simple_default && default_is_simple(call.default) {
            return;
        }
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
            // `map_or_default`, which is suggested for this
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
