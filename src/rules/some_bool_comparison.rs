use crate::common::{DefaultState, hir_in_external_macro};
use crate::rule_index::{Register, rule};
use clippy_utils::diagnostics::span_lint_and_sugg;
use clippy_utils::sugg::Sugg;
use clippy_utils::{as_some_expr, peel_hir_expr_refs};
use rustc_ast::LitKind;
use rustc_errors::Applicability;
use rustc_hir::{BinOpKind, Expr, ExprKind, Mutability};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::sym;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags an `Option<bool>` compared with `==` or `!=` against a
    /// `Some` of a boolean literal, and names the `unwrap_or` that says
    /// what `None` means:
    ///
    /// | Comparison            | Prefer                  |
    /// |-----------------------|-------------------------|
    /// | `opt == Some(true)`   | `opt.unwrap_or(false)`  |
    /// | `opt != Some(false)`  | `opt.unwrap_or(true)`   |
    /// | `opt != Some(true)`   | `!opt.unwrap_or(false)` |
    /// | `opt == Some(false)`  | `!opt.unwrap_or(true)`  |
    ///
    /// Either operand may be the `Some`, and either may be borrowed, so
    /// `Some(true) == opt` and `opt == &Some(true)` are flagged the same.
    /// A borrowed payload — what `HashMap::<_, bool>::get` and
    /// `Option::as_ref` hand back — gains a `copied` to reach the `bool`,
    /// so `map.get(k) == Some(&true)` becomes
    /// `map.get(k).copied().unwrap_or(false)`.
    ///
    /// Left alone:
    ///
    /// - `matches!(opt, Some(true))`. It already names the state it
    ///   matches, which is what this rule asks for.
    /// - A `Some` carrying a variable. There is no state to name, and
    ///   `unwrap_or` would not be an improvement.
    /// - A `Some` an expansion produced, as in `opt == wanted!()`. The
    ///   rewrite reads the literal and drops the call, which would
    ///   freeze today's expansion into the source.
    /// - A comparison a macro builds, `assert_eq!(opt, Some(true))`
    ///   among them. An equality assertion already names the state it
    ///   expects, and rewriting it would cost its failure message the
    ///   operand values it prints. A comparison written as a macro
    ///   *argument* is the author's own, so `assert!(opt == Some(true))`
    ///   is flagged.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. The
    /// comparison is exactly equivalent to its replacement. The
    /// objection is to what the reader has to do:
    ///
    /// - **The `None` case is left implicit.** `== Some(true)` never
    ///   says what `None` means, so the reader derives it from the
    ///   operator. `unwrap_or(false)` states it — absent counts as
    ///   false — which is usually the decision the surrounding code
    ///   turned on.
    /// - **`!=` compounds that.** `opt != Some(true)` admits two states
    ///   for two different reasons, and which two is not apparent
    ///   without working through the cases.
    /// - **It reads as a value test, not a presence test.** Nothing at
    ///   the call site distinguishes `Option<bool>` from `bool`, so a
    ///   reader scanning a function for its option-handling can pass
    ///   straight over it.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::bool_comparison` rewrites `b == true` to `b` for a plain
    /// `bool` and stops there. None of `opt == Some(true)`,
    /// `opt == Some(false)`, `opt != Some(true)` or
    /// `map.get(k) == Some(&true)` produces a diagnostic from it, so
    /// this rule is that lint's complement on `Option<bool>` rather than
    /// a refinement of it.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// if settings.verbose == Some(true) { /* ... */ }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// if settings.verbose.unwrap_or(false) { /* ... */ }
    /// ```
    pub perfectionist::SOME_BOOL_COMPARISON,
    Warn,
    "`Option<bool>` compared against a `Some` of a boolean literal, which never says what `None` means",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::some_bool_comparison";

/// The rule has no configuration knobs. Not dead code: the read
/// below rejects a mistyped key in the rule's `dylint.toml` table,
/// and gen-docs needs the struct for `Configuration: none.`
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {}

pub struct SomeBoolComparison;

impl_lint_pass!(SomeBoolComparison => [SOME_BOOL_COMPARISON]);

impl Register for rule::SomeBoolComparison {
    /// One expression shape with a total, value-preserving rewrite and
    /// nothing to tune, so there is no baseline a project could want
    /// instead.
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[SOME_BOOL_COMPARISON]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        lint_store.register_late_lint_pass(Box::new(|_| Box::new(SomeBoolComparison)));
    }
}

/// Which payload the option side carries, and so what the rewrite has
/// to put between it and the `unwrap_or`.
#[derive(Clone, Copy)]
enum Payload {
    /// `Option<bool>`, which `unwrap_or` already answers with a `bool`.
    Owned,
    /// `Option<&bool>`, where `unwrap_or` would answer with a `&bool`.
    Borrowed,
}

impl Payload {
    /// The call the rewrite inserts ahead of the `unwrap_or`.
    fn reaching_the_bool(self) -> &'static str {
        match self {
            Payload::Owned => "",
            Payload::Borrowed => ".copied()",
        }
    }
}

/// The parts of a flagged comparison the rewrite is built from.
struct Comparison<'tcx> {
    /// The operand whose state the comparison describes, with any outer
    /// `&` peeled off: what the rewrite calls `unwrap_or` on.
    option: &'tcx Expr<'tcx>,
    /// The boolean the other operand's `Some` carries.
    literal: bool,
    payload: Payload,
}

/// What the rewrite does to the option side: the default its `unwrap_or`
/// takes, and whether the call is negated.
struct Rewrite {
    default: bool,
    negated: bool,
}

/// How the comparison is rewritten, given that it was written with
/// `equality` — `==` rather than `!=` — against `literal`.
///
/// The default is the literal's opposite throughout, which is what makes
/// the call answer for `None` the way the comparison did; the negation
/// is what makes it answer for `Some` too. The unit test beside this file
/// holds the pair to the comparison's own answer in every state.
fn rewrite(equality: bool, literal: bool) -> Rewrite {
    Rewrite {
        default: !literal,
        negated: equality != literal,
    }
}

impl<'tcx> LateLintPass<'tcx> for SomeBoolComparison {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::Binary(operator, left, right) = expr.kind else {
            return;
        };
        let equality = match operator.node {
            BinOpKind::Eq => true,
            BinOpKind::Ne => false,
            _ => return,
        };
        // A comparison a macro built is the macro author's rather than
        // the caller's, and the suggestion would rewrite the definition
        // with one call site's text. `report_in_external_macro: false`
        // covers only another crate's macro, so a same-crate
        // `macro_rules!` needs this. A comparison written as a macro
        // argument carries the author's own span and is reported.
        if expr.span.from_expansion() {
            return;
        }
        let Some(comparison) = comparison(cx, left, right).or_else(|| comparison(cx, right, left))
        else {
            return;
        };
        // A derive may stamp a user span on every token of a comparison
        // it synthesises, which leaves the check above nothing to find;
        // the enclosing item's span is what still says where the
        // comparison lives.
        if hir_in_external_macro(cx, expr.hir_id, expr.span) {
            return;
        }
        emit(cx, expr, &comparison, equality);
    }
}

/// The comparison reading `some` as the `Some` of a boolean literal and
/// `option` as the side whose state it describes.
fn comparison<'tcx>(
    cx: &LateContext<'tcx>,
    some: &'tcx Expr<'tcx>,
    option: &'tcx Expr<'tcx>,
) -> Option<Comparison<'tcx>> {
    let literal = some_bool_literal(cx, some)?;
    let option = peel_hir_expr_refs(option).0;
    Some(Comparison {
        option,
        literal,
        payload: option_bool_payload(cx, option)?,
    })
}

/// The boolean a `Some` of a boolean literal carries, where the author
/// wrote that literal. Any outer `&` comes off the call and off its
/// argument — the two peels that admit `&Some(true)` and `Some(&true)`.
///
/// A literal out of an expansion answers `None`. The rewrite reads its
/// value and drops its text, so `opt == wanted!()` would otherwise be
/// rewritten to whatever `wanted!()` expands to today. The whole
/// comparison's span stays the author's in that shape, so the bail on it
/// does not reach this.
fn some_bool_literal(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<bool> {
    let argument = as_some_expr(cx, peel_hir_expr_refs(expr).0)?;
    let argument = peel_hir_expr_refs(argument).0;
    let ExprKind::Lit(literal) = argument.kind else {
        return None;
    };
    if argument.span.from_expansion() {
        return None;
    }
    match literal.node {
        LitKind::Bool(value) => Some(value),
        _ => None,
    }
}

/// Which payload `expr`'s type carries, where that type is an `Option`
/// of a `bool` or of a `&bool`.
///
/// References around the `Option` itself are peeled, so a
/// `&Option<bool>` answers as the `Option<bool>` it derefs to — which is
/// what the rewrite's `unwrap_or` resolves against too.
fn option_bool_payload(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<Payload> {
    let ty::Adt(option, arguments) = cx.typeck_results().expr_ty(expr).peel_refs().kind() else {
        return None;
    };
    if !cx.tcx.is_diagnostic_item(sym::Option, option.did()) {
        return None;
    }
    match arguments.type_at(0).kind() {
        ty::Bool => Some(Payload::Owned),
        ty::Ref(_, payload, Mutability::Not) if payload.is_bool() => Some(Payload::Borrowed),
        _ => None,
    }
}

/// Report the comparison and hand over the `unwrap_or` that names the
/// state it left implicit.
fn emit<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    comparison: &Comparison<'tcx>,
    equality: bool,
) {
    let Rewrite { default, negated } = rewrite(equality, comparison.literal);
    let mut applicability = Applicability::MachineApplicable;
    // The option side becomes a method-call receiver, so one that binds
    // looser keeps its own brackets: `*p` spliced raw reads as
    // `*p.unwrap_or(false)`, which derefs the result and does not
    // compile.
    let receiver =
        Sugg::hir_with_applicability(cx, comparison.option, "..", &mut applicability).maybe_paren();
    span_lint_and_sugg(
        cx,
        SOME_BOOL_COMPARISON,
        expr.span,
        format!(
            "comparing against `Some({})` leaves the `None` case unnamed",
            comparison.literal,
        ),
        "name it with `unwrap_or`",
        format!(
            "{}{receiver}{}.unwrap_or({default})",
            if negated { "!" } else { "" },
            comparison.payload.reaching_the_bool(),
        ),
        applicability,
    );
}

#[cfg(test)]
mod tests;
