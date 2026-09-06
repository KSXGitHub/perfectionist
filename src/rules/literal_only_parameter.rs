use crate::common::DefaultState;
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_hir_and_then;
use clippy_utils::is_lang_item_or_ctor;
use rustc_hir as hir;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Expr, ExprKind, HirId, LangItem};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::{self, AssocContainer};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol};
use std::collections::{BTreeMap, HashMap, HashSet};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a `bool` or `Option` parameter of a function that every
    /// call site in the crate passes as a literal — `true`, `false`,
    /// `None`, or `Some(..)` — and never as a value it computed.
    ///
    /// When every call passes the same literal, the parameter is a
    /// constant and the diagnostic says to drop it. When the calls
    /// differ, the function is two functions sharing a body, and the
    /// diagnostic says to split it.
    ///
    /// Only a function whose callers are all in the crate can be
    /// judged, so a function reachable from the crate's public API is
    /// left alone, as is a trait method, a function that is never
    /// called, a function that is also used as a value (passed to
    /// `map`, stored in a struct), and a function produced by a macro.
    /// An argument produced by a macro expansion counts as computed.
    ///
    /// Test code is judged like any other code; set
    /// `test_code_exception` to leave it alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A
    /// parameter that is only ever a literal is not a parameter but a
    /// mode switch: inside the function an `if` on it picks one of two
    /// control flows that no caller ever wants to choose at run time.
    /// The two flows share a name and a body while depending on each
    /// other's branches, and each caller reads a `true` or `false` with
    /// no idea what it selects. Splitting the function gives each flow
    /// a name, removes the branch, and moves what the two really share
    /// into a helper both call. rust-analyzer's style guide calls the
    /// shape "false sharing".
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::fn_params_excessive_bools` (`pedantic`) counts the
    /// `bool` parameters of a signature regardless of how they are
    /// called. This rule looks at the calls, so a single `bool` that
    /// every caller hard-codes is caught while a `bool` a caller really
    /// computes is not.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// fn render(entries: &[Entry], with_sizes: bool) -> String {
    ///     let mut out = String::new();
    ///     for entry in entries {
    ///         out.push_str(&entry.name);
    ///         if with_sizes {
    ///             out.push_str(&format!(" ({})", entry.size));
    ///         }
    ///         out.push('\n');
    ///     }
    ///     out
    /// }
    ///
    /// fn list(entries: &[Entry]) -> String { render(entries, false) }
    /// fn list_verbose(entries: &[Entry]) -> String { render(entries, true) }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// fn list(entries: &[Entry]) -> String {
    ///     render(entries, |entry| entry.name.clone())
    /// }
    ///
    /// fn list_verbose(entries: &[Entry]) -> String {
    ///     render(entries, |entry| format!("{} ({})", entry.name, entry.size))
    /// }
    ///
    /// fn render(entries: &[Entry], line: impl Fn(&Entry) -> String) -> String {
    ///     entries.iter().map(|entry| line(entry) + "\n").collect()
    /// }
    /// ```
    pub perfectionist::LITERAL_ONLY_PARAMETER,
    Warn,
    "`bool` or `Option` parameter that every call site passes as a literal",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::literal_only_parameter";

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Whether test code is left alone: a function inside a
    /// `#[cfg(test)]` module, a `#[test]` function, or an
    /// integration-test or benchmark target. Calls made from test code
    /// to a production function still count as call sites. Defaults to
    /// `false`.
    test_code_exception: bool,
}

/// What a call site passed for one parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Argument {
    True,
    False,
    None,
    Some,
    Computed,
}

impl Argument {
    fn literal_text(self) -> &'static str {
        match self {
            Argument::True => "`true`",
            Argument::False => "`false`",
            Argument::None => "`None`",
            Argument::Some => "`Some(..)`",
            Argument::Computed => "a computed value",
        }
    }
}

/// A `bool` or `Option` parameter of a function under judgement.
struct Parameter {
    /// Position among the body's parameters, `self` included.
    index: usize,
    name: Symbol,
    span: Span,
}

struct Candidate {
    name: Symbol,
    hir_id: HirId,
    parameters: Vec<Parameter>,
    /// One entry per call site, each the arguments aligned to the
    /// body's parameters.
    calls: Vec<Vec<Argument>>,
    /// The function was named without being called, so callers we
    /// cannot see may pass anything.
    used_as_value: bool,
}

pub struct LiteralOnlyParameter {
    config: Config,
    candidates: HashMap<LocalDefId, Candidate>,
    /// Callee expressions of calls already recorded, so the path
    /// expression a call goes through is not also taken for a use as
    /// a value.
    callees: HashSet<HirId>,
}

impl_lint_pass!(LiteralOnlyParameter => [LITERAL_ONLY_PARAMETER]);

impl Register for rule::LiteralOnlyParameter {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[LITERAL_ONLY_PARAMETER]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(LiteralOnlyParameter {
                config: dylint_linting::config_or_default(CONFIG_KEY),
                candidates: HashMap::new(),
                callees: HashSet::new(),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for LiteralOnlyParameter {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _span: Span,
        def_id: LocalDefId,
    ) {
        let (FnKind::ItemFn(ident, ..) | FnKind::Method(ident, ..)) = kind else {
            return;
        };
        if cx.tcx.def_span(def_id).from_expansion() {
            return;
        }
        if cx.tcx.effective_visibilities(()).is_exported(def_id) {
            return;
        }
        // A trait fixes the signature of its methods.
        if let Some(assoc) = cx.tcx.opt_associated_item(def_id.to_def_id())
            && !matches!(assoc.container, AssocContainer::InherentImpl)
        {
            return;
        }
        if self.config.test_code_exception && item_in_test_code(cx, def_id) {
            return;
        }
        let inputs = cx
            .tcx
            .fn_sig(def_id)
            .instantiate_identity()
            .skip_binder()
            .inputs();
        let parameters: Vec<Parameter> = body
            .params
            .iter()
            .zip(inputs)
            .enumerate()
            .filter(|(_, (_, input))| is_bool_or_option(cx, **input))
            .filter_map(|(index, (param, _))| {
                let hir::PatKind::Binding(_, _, ident, None) = param.pat.kind else {
                    return None;
                };
                Some(Parameter {
                    index,
                    name: ident.name,
                    span: param.span,
                })
            })
            .collect();
        if parameters.is_empty() {
            return;
        }
        self.candidates.insert(
            def_id,
            Candidate {
                name: ident.name,
                hir_id: cx.tcx.local_def_id_to_hir_id(def_id),
                parameters,
                calls: Vec::new(),
                used_as_value: false,
            },
        );
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        match expr.kind {
            ExprKind::Call(callee, args) => {
                let ExprKind::Path(qpath) = &callee.kind else {
                    return;
                };
                let Some(def_id) = local_fn(cx.qpath_res(qpath, callee.hir_id)) else {
                    return;
                };
                self.callees.insert(callee.hir_id);
                self.record_call(cx, def_id, None, args);
            }
            ExprKind::MethodCall(_, receiver, args, _) => {
                let Some(def_id) = cx
                    .typeck_results()
                    .type_dependent_def_id(expr.hir_id)
                    .and_then(|id| id.as_local())
                else {
                    return;
                };
                self.record_call(cx, def_id, Some(receiver), args);
            }
            ExprKind::Path(qpath) => {
                if self.callees.contains(&expr.hir_id) {
                    return;
                }
                if let Some(def_id) = local_fn(cx.qpath_res(&qpath, expr.hir_id))
                    && let Some(candidate) = self.candidates.get_mut(&def_id)
                {
                    candidate.used_as_value = true;
                }
            }
            _ => {}
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let mut candidates: Vec<_> = self.candidates.drain().collect();
        candidates.sort_by_key(|(_, candidate)| candidate.parameters[0].span);
        for (_, candidate) in candidates {
            if candidate.used_as_value || candidate.calls.is_empty() {
                continue;
            }
            for parameter in &candidate.parameters {
                let mut tally: BTreeMap<Argument, usize> = BTreeMap::new();
                for call in &candidate.calls {
                    let argument = call
                        .get(parameter.index)
                        .copied()
                        .unwrap_or(Argument::Computed);
                    *tally.entry(argument).or_default() += 1;
                }
                if tally.contains_key(&Argument::Computed) {
                    continue;
                }
                emit(cx, &candidate, parameter, &tally);
            }
        }
    }
}

impl LiteralOnlyParameter {
    /// Record one call of `def_id`, aligning `args` to the body's
    /// parameters: a method call's receiver is parameter zero.
    fn record_call(
        &mut self,
        cx: &LateContext<'_>,
        def_id: LocalDefId,
        receiver: Option<&Expr<'_>>,
        args: &[Expr<'_>],
    ) {
        let Some(candidate) = self.candidates.get_mut(&def_id) else {
            return;
        };
        let arguments = receiver
            .into_iter()
            .chain(args)
            .map(|arg| classify(cx, arg))
            .collect();
        candidate.calls.push(arguments);
    }
}

/// The local function or inherent method `res` names, if any.
fn local_fn(res: Res) -> Option<LocalDefId> {
    let Res::Def(DefKind::Fn | DefKind::AssocFn, def_id) = res else {
        return None;
    };
    def_id.as_local()
}

fn is_bool_or_option(cx: &LateContext<'_>, ty: ty::Ty<'_>) -> bool {
    match ty.kind() {
        ty::Bool => true,
        ty::Adt(def, _) => cx.tcx.is_lang_item(def.did(), LangItem::Option),
        _ => false,
    }
}

/// What `arg` is at its call site.
fn classify(cx: &LateContext<'_>, arg: &Expr<'_>) -> Argument {
    if arg.span.from_expansion() {
        return Argument::Computed;
    }
    match arg.kind {
        ExprKind::Lit(lit) => match lit.node {
            rustc_ast::LitKind::Bool(true) => Argument::True,
            rustc_ast::LitKind::Bool(false) => Argument::False,
            _ => Argument::Computed,
        },
        ExprKind::Path(qpath) => match cx.qpath_res(&qpath, arg.hir_id) {
            Res::Def(DefKind::Ctor(..), def_id)
                if is_lang_item_or_ctor(cx, def_id, LangItem::OptionNone) =>
            {
                Argument::None
            }
            _ => Argument::Computed,
        },
        ExprKind::Call(callee, [_]) => {
            let ExprKind::Path(qpath) = &callee.kind else {
                return Argument::Computed;
            };
            match cx.qpath_res(qpath, callee.hir_id) {
                Res::Def(DefKind::Ctor(..), def_id)
                    if is_lang_item_or_ctor(cx, def_id, LangItem::OptionSome) =>
                {
                    Argument::Some
                }
                _ => Argument::Computed,
            }
        }
        _ => Argument::Computed,
    }
}

fn emit(
    cx: &LateContext<'_>,
    candidate: &Candidate,
    parameter: &Parameter,
    tally: &BTreeMap<Argument, usize>,
) {
    let function = candidate.name;
    let name = parameter.name;
    let (message, help) = if let [(only, _)] = tally.iter().collect::<Vec<_>>()[..] {
        let literal = only.literal_text();
        (
            format!("parameter `{name}` of `{function}` is always passed {literal}"),
            "drop the parameter and inline the value it always has",
        )
    } else {
        let breakdown: Vec<String> = tally
            .iter()
            .map(|(argument, count)| {
                let literal = argument.literal_text();
                let noun = if *count == 1 {
                    "call site"
                } else {
                    "call sites"
                };
                format!("{literal} at {count} {noun}")
            })
            .collect();
        let breakdown = breakdown.join(", ");
        (
            format!(
                "parameter `{name}` of `{function}` is only ever passed a literal: {breakdown}",
            ),
            "split the function into one per case and move what the cases share into a helper",
        )
    };
    span_lint_hir_and_then(
        cx,
        LITERAL_ONLY_PARAMETER,
        candidate.hir_id,
        parameter.span,
        message,
        |diag| {
            diag.help(help);
        },
    );
}
