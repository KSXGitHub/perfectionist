//! `perfectionist::single_letter_closure_param` — flag closure
//! parameters whose identifier is one ASCII letter, unless the
//! closure is a trivial single-expression callback.
//!
//! The trivial-callback predicate lives in [`triviality`]; this file
//! owns the lint declaration, the configuration, and the late pass
//! that drives them.

use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

use rustc_span::Symbol;

use crate::common::{
    DefaultState, binding_ident, hir_in_external_macro, is_single_ascii_letter, resolve_symbol_set,
    resolved_state,
};

mod triviality;

use triviality::{is_trivial_wrapper, parent_call_callee_name, single_expression_body};

declare_tool_lint! {
    /// ### What it does
    /// Flags closure parameters whose identifier is one ASCII
    /// letter, unless the closure is a trivial single-expression
    /// callback or the identifier is in the conventional-name
    /// exempt set (`n` for an unsigned count, `f` for a
    /// `fmt::Formatter`, `i` / `j` / `k` for indices).
    /// "Single-expression" is a shared precondition for the
    /// trivial-callback exception: the body must be a bare
    /// expression or a block whose only content is a trailing
    /// expression — a body with any `let` binding or other
    /// statement before the trailing expression disqualifies the
    /// closure regardless of which branch below would otherwise
    /// apply. Given that, one of two further shapes must hold:
    /// - the closure is the immediate argument of a call whose
    ///   callee name is in the trivial-callback method set
    ///   (`sort_by`, `sort_by_key`, `min_by`, `max_by`,
    ///   `binary_search_by`, `cmp_by`, `partial_cmp_by`,
    ///   `fold`, `try_fold`, …). The set also covers the
    ///   matching adaptors from `itertools` (`sorted_by`,
    ///   `k_smallest_by`, `minmax_by_key`, …) and `into-sorted`
    ///   (`into_sorted_by`, `into_sorted_by_key`, …);
    /// - the body is a trivial wrapper around the parameter —
    ///   a field access (`|x| x.field`), a method call
    ///   (`|x| x.foo()`), a one-argument call where the
    ///   parameter is the sole argument (`|x| f(x)`), a
    ///   reference (`|x| &x`), or a macro call
    ///   (`|x| vec![x]`, `|x| dbg!(x)`,
    ///   `|x| format!("{x}")`). Surrounding `*` / `&`
    ///   operators around the parameter inside any of the
    ///   non-macro shapes are peeled before the match, so
    ///   `|s| (*s).foo()` qualifies.
    ///
    /// The conventional-name exempt set matches the one used by
    /// `perfectionist::single_letter_function_param`: `|i| ...`
    /// is the canonical index closure, just as `fn step(i: usize)`
    /// is the canonical index parameter. Bodies that use the
    /// index for slicing or arithmetic (`|i| &hex[i..i + 2]`)
    /// are not structurally trivial, so the exempt set is what
    /// keeps them out of the diagnostic.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// A multi-line closure body whose parameter is a single
    /// letter forces the reader to scroll back to the closure
    /// header for context on every reference. The
    /// trivial-callback exception covers `sort_by(|a, b| ...)` and
    /// `.map(|x| x.field)` shapes that are short enough that the
    /// parameter's role is unambiguous from the call site.
    ///
    /// ### Example
    /// ```rust,ignore
    /// .map(|t| {
    ///     let columns = build_columns(t);
    ///     format_row(&columns)
    /// })
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// .map(|tree_row| {
    ///     let columns = build_columns(tree_row);
    ///     format_row(&columns)
    /// })
    /// ```
    pub perfectionist::SINGLE_LETTER_CLOSURE_PARAM,
    Warn,
    "closure parameter has a single-letter name",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::single_letter_closure_param";

/// Default exempt identifiers for closure parameter names,
/// mirroring the `single_letter_function_param` defaults: `n` for
/// an unsigned count, `f` for a `fmt::Formatter`, and `i` / `j`
/// / `k` for indices. Closures use the same conventional
/// one-letter names as function and method signatures; an
/// `|i| &hex[i..i + 2]` indexing closure is just as canonical as
/// a `fn step(i: usize)` signature, and is not covered by the
/// structural trivial-wrapper shapes.
const DEFAULT_CLOSURE_PARAM_EXEMPTIONS: &[&str] = &["n", "f", "i", "j", "k"];

/// Default trivial-callback method names whose closure argument
/// may use single-letter parameters when the body is a single
/// expression. Both source documents agree on this set.
const DEFAULT_TRIVIAL_CALLBACK_METHODS: &[&str] = &[
    "sort_by",
    "sort_unstable_by",
    "sort_by_key",
    "sort_unstable_by_key",
    "min_by",
    "max_by",
    "min_by_key",
    "max_by_key",
    "binary_search_by",
    "binary_search_by_key",
    // `Iterator::cmp_by` / `partial_cmp_by` / `eq_by` take a
    // closure of two parameters. The bare `cmp` / `partial_cmp`
    // trait methods are not closure-accepting, so they are not
    // listed here.
    "cmp_by",
    "partial_cmp_by",
    "eq_by",
    "fold",
    "try_fold",
    "rfold",
    "reduce",
    // `itertools::Itertools` adaptors that take a comparator or
    // key closure and follow the same `|a, b| ...` / `|x| ...`
    // convention as their std counterparts.
    "sorted_by",
    "sorted_by_key",
    "sorted_by_cached_key",
    "sorted_unstable_by",
    "sorted_unstable_by_key",
    "k_smallest_by",
    "k_smallest_by_key",
    "k_smallest_relaxed_by",
    "k_smallest_relaxed_by_key",
    "k_largest_by",
    "k_largest_by_key",
    "k_largest_relaxed_by",
    "k_largest_relaxed_by_key",
    "min_set_by",
    "min_set_by_key",
    "max_set_by",
    "max_set_by_key",
    "minmax_by",
    "minmax_by_key",
    "position_min_by",
    "position_min_by_key",
    "position_max_by",
    "position_max_by_key",
    "position_minmax_by",
    "position_minmax_by_key",
    // `into_sorted::{IntoSorted, IntoSortedUnstable}` adaptors
    // that return an owned sorted array, mirroring the std
    // `sort_by` / `sort_by_key` shapes.
    "into_sorted_by",
    "into_sorted_by_key",
    "into_sorted_by_cached_key",
    "into_sorted_unstable_by",
    "into_sorted_unstable_by_key",
];

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Additional method / function names whose closure argument
    /// may carry single-letter parameters when the body is a
    /// single expression. Merged with the built-in defaults (the
    /// curated `core` / `std` callbacks plus selected `itertools`
    /// and `into-sorted` adaptors); empty by default. List
    /// project-specific DSL helpers (`when`, `iter_by`, third-party
    /// callbacks such as `into_sorted_by`, …) here without having
    /// to re-state the standard ones.
    extra_trivial_callback_methods: Vec<String>,
    /// Method / function names to drop from the trivial-callback
    /// set, even if they appear in the built-in defaults or in
    /// `extra_trivial_callback_methods`. Empty by default; checked
    /// after the merge with the built-ins, so this knob always
    /// wins. Useful for opting back into linting on a default
    /// entry the project does not consider trivial.
    ignore_trivial_callback_methods: Vec<String>,
    /// Additional identifiers to allow as closure parameter names.
    /// Merged with the built-in defaults
    /// (`["n", "f", "i", "j", "k"]`); empty by default. Use this
    /// to whitelist project-specific conventional names without
    /// having to re-state the standard ones.
    extra_allowed_idents: Vec<String>,
    /// Identifiers to drop from the exempt set, even if they
    /// appear in the built-in defaults or in
    /// `extra_allowed_idents`. Empty by default; checked after
    /// the merge with the built-ins, so this knob always wins.
    ignore_allowed_idents: Vec<String>,
}

pub struct SingleLetterClosureParam {
    trivial_callback_methods: BTreeSet<Symbol>,
    allowed_idents: BTreeSet<Symbol>,
}

impl SingleLetterClosureParam {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let trivial_callback_methods = resolve_symbol_set(
            DEFAULT_TRIVIAL_CALLBACK_METHODS,
            config.extra_trivial_callback_methods,
            config.ignore_trivial_callback_methods,
        );
        let allowed_idents = resolve_symbol_set(
            DEFAULT_CLOSURE_PARAM_EXEMPTIONS,
            config.extra_allowed_idents,
            config.ignore_allowed_idents,
        );
        Self {
            trivial_callback_methods,
            allowed_idents,
        }
    }
}

impl_lint_pass!(SingleLetterClosureParam => [SINGLE_LETTER_CLOSURE_PARAM]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[SINGLE_LETTER_CLOSURE_PARAM]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive =
        resolved_state("single_letter_closure_param", DefaultState::Active)
    {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(SingleLetterClosureParam::new()));
}

impl<'tcx> LateLintPass<'tcx> for SingleLetterClosureParam {
    fn check_expr(&mut self, lint_context: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        let hir::ExprKind::Closure(closure) = expr.kind else {
            return;
        };
        if hir_in_external_macro(lint_context, expr.hir_id, expr.span) {
            return;
        }
        let body = lint_context.tcx.hir_body(closure.body);
        let single_letter_params: Vec<rustc_span::Ident> = body
            .params
            .iter()
            .filter_map(|param| {
                let ident = binding_ident(param.pat)?;
                is_single_ascii_letter(ident.name.as_str()).then_some(ident)
            })
            .filter(|ident| !self.allowed_idents.contains(&ident.name))
            .collect();
        if single_letter_params.is_empty() {
            return;
        }
        if self.closure_is_trivial(lint_context, expr, body) {
            return;
        }
        for ident in single_letter_params {
            span_lint_and_help(
                lint_context,
                SINGLE_LETTER_CLOSURE_PARAM,
                ident.span,
                format!(
                    "closure parameter `{}` has a single-letter name",
                    ident.name,
                ),
                None,
                "rename to a descriptive identifier, or rewrite the closure as \
                 a trivial single-expression callback",
            );
        }
    }
}

impl SingleLetterClosureParam {
    fn closure_is_trivial<'tcx>(
        &self,
        lint_context: &LateContext<'tcx>,
        closure_expr: &'tcx hir::Expr<'tcx>,
        body: &'tcx hir::Body<'tcx>,
    ) -> bool {
        let Some(body_expr) = single_expression_body(body) else {
            return false;
        };
        if self.parent_call_is_trivial_callback(lint_context, closure_expr) {
            return true;
        }
        if is_trivial_wrapper(body_expr, body.params) {
            return true;
        }
        false
    }

    fn parent_call_is_trivial_callback<'tcx>(
        &self,
        lint_context: &LateContext<'tcx>,
        closure_expr: &'tcx hir::Expr<'tcx>,
    ) -> bool {
        let Some(name) = parent_call_callee_name(lint_context, closure_expr) else {
            return false;
        };
        self.trivial_callback_methods.contains(&name)
    }
}
