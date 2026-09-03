use crate::cargo_target::{CargoTarget, crate_target};
use crate::common::{
    DefaultState, binding_hir_id, binding_ident, hir_in_external_macro, resolved_state,
};
use crate::test_code::in_test_code;
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::source::{snippet, snippet_opt};
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{Expr, ExprKind, HirId, Node, QPath};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::{self, AssocContainer, Ty, TyCtxt};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Symbol, sym};

mod config;

use config::{Config, Resolved};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a function or method parameter taken by shared reference
    /// (`&str`, `&Path`, `&OsStr`, `&CStr`, `&[T]`) whose *only* use in
    /// the body is to produce its owned counterpart (`String`,
    /// `PathBuf`, `OsString`, `CString`, `Vec<T>`) — via `to_owned`,
    /// `to_string`, `to_path_buf`, `to_vec`, `to_os_string`, `clone`,
    /// `into`, or `String::from(..)` and friends. Such a parameter
    /// should take the owned form directly.
    ///
    /// Only the conservative single-use case is flagged: the
    /// parameter must be referenced exactly once, that use must be the
    /// conversion, and the conversion must execute unconditionally — no
    /// `if` / `match` / loop, closure, or short-circuiting `&&` / `||`
    /// may sit between it and the enclosing function. This is
    /// deliberately conservative: even the always-executed `if`
    /// condition and `match` scrutinee positions count as
    /// disqualifying, not just the branch arms.
    ///
    /// Test code and build scripts are exempt by default;
    /// `test_code_exception` and `build_script_exception` turn either
    /// off.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Taking
    /// the borrowed form and converting inside the body forces a copy
    /// even when the caller already owns the value. Taking the owned
    /// form moves the caller's value straight in; a caller that holds
    /// only a borrow performs the very same conversion at the call site
    /// that the borrowed signature was performing internally. The total
    /// number of copies is identical for the worst caller and one
    /// cheaper for the best caller.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::needless_pass_by_value` (`pedantic`, off by default)
    /// covers the *opposite* direction: a by-value parameter that is
    /// never consumed should be taken by reference. `clippy::ptr_arg`
    /// (`style`, on by default) is orthogonal — it rewrites a
    /// `&String` / `&Vec<T>` / `&PathBuf` parameter to the more
    /// permissive `&str` / `&[T]` / `&Path`. Neither moves a borrowed
    /// parameter to its owned form, so enabling all three gives full
    /// coverage of the owned-vs-borrowed trade-off.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// fn push_path(list: &mut Vec<PathBuf>, item: &Path) {
    ///     list.push(item.to_path_buf());
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// fn push_path(list: &mut Vec<PathBuf>, item: PathBuf) {
    ///     list.push(item);
    /// }
    /// ```
    pub perfectionist::NEEDLESS_BORROWED_PARAMETERS,
    Warn,
    "borrowed parameter is only used to produce its owned form",
    report_in_external_macro: false
}

/// Active by default. Read by [`register_pass`] below; gen-docs picks
/// the constant up via syn to render the rule's default state.
pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Active;

const CONFIG_KEY: &str = "perfectionist::needless_borrowed_parameters";

pub struct NeedlessBorrowedParameters {
    config: Resolved,
    /// Whether the whole crate under compilation is exempt, memoised
    /// on first use: it depends only on which Cargo target this is, so
    /// it is the same answer for every function in the crate.
    exempt_crate: Option<bool>,
}

impl NeedlessBorrowedParameters {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            config: Resolved::from_config(config),
            exempt_crate: None,
        }
    }
}

impl_lint_pass!(NeedlessBorrowedParameters => [NEEDLESS_BORROWED_PARAMETERS]);

/// Register this rule's lint declaration. Paired with [`register_pass`];
/// see the module-level convention documented in `register_lints`.
pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[NEEDLESS_BORROWED_PARAMETERS]);
}

/// Install this rule's late pass.
pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("needless_borrowed_parameters", DEFAULT_STATE) {
        return;
    }
    lint_store.register_late_lint_pass(Box::new(|_| Box::new(NeedlessBorrowedParameters::new())));
}

impl<'tcx> LateLintPass<'tcx> for NeedlessBorrowedParameters {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _span: rustc_span::Span,
        def_id: rustc_span::def_id::LocalDefId,
    ) {
        if matches!(kind, FnKind::Closure) {
            return;
        }
        if self.is_exempt(cx, def_id) {
            return;
        }
        // A signature dictated by a trait — whether the trait's own
        // declaration or an `impl Trait for T` method — cannot be
        // changed by this rule's reader, so leave it alone. Only free
        // functions and inherent methods are eligible.
        if let Some(assoc) = cx.tcx.opt_associated_item(def_id.to_def_id())
            && !matches!(assoc.container, AssocContainer::InherentImpl)
        {
            return;
        }

        let sig = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        let inputs = sig.inputs();
        let typeck = cx.tcx.typeck(def_id);

        for (index, param) in body.params.iter().enumerate() {
            let (Some(param_ty), Some(hir_ty)) = (inputs.get(index), decl.inputs.get(index)) else {
                continue;
            };
            self.check_param(cx, typeck, body, param, *param_ty, hir_ty);
        }
    }
}

impl NeedlessBorrowedParameters {
    /// Whether the function at `def_id` sits in code the rule stays
    /// out of, per the `test_code_exception` / `build_script_exception`
    /// knobs.
    ///
    /// The crate-wide half — an integration-test, benchmark, or
    /// build-script target, where *every* function is exempt — is
    /// memoised, since it is one answer per crate. The per-function
    /// half walks the enclosing scopes for a `#[cfg(test)]` gate or a
    /// `#[test]` function, so it is asked afresh each time.
    fn is_exempt(&mut self, cx: &LateContext<'_>, def_id: rustc_span::def_id::LocalDefId) -> bool {
        let test_code_exception = self.config.test_code_exception;
        let build_script_exception = self.config.build_script_exception;
        let exempt_crate = *self
            .exempt_crate
            .get_or_insert_with(|| match crate_target(cx) {
                CargoTarget::BuildScript => build_script_exception,
                target => test_code_exception && target.is_test_target(),
            });
        exempt_crate
            || (test_code_exception && in_test_code(cx.tcx, cx.tcx.local_def_id_to_hir_id(def_id)))
    }

    fn check_param<'tcx>(
        &self,
        cx: &LateContext<'tcx>,
        typeck: &ty::TypeckResults<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        param: &'tcx hir::Param<'tcx>,
        param_ty: Ty<'tcx>,
        hir_ty: &'tcx hir::Ty<'tcx>,
    ) {
        // The written form must be a shared reference with an elided
        // lifetime. An explicit named lifetime may tie the parameter to
        // another parameter or the return type, where moving to an owned
        // value would break the lifetime contract.
        let hir::TyKind::Ref(lifetime, mut_ty) = hir_ty.kind else {
            return;
        };
        if mut_ty.mutbl != hir::Mutability::Not || !lifetime.is_anonymous() {
            return;
        }
        let ty::Ref(_, pointee, _) = param_ty.kind() else {
            return;
        };
        let Some(owned) = owned_counterpart(cx, *pointee, mut_ty.ty) else {
            return;
        };

        let Some(target) = binding_hir_id(param.pat) else {
            return;
        };
        let Some(ident) = binding_ident(param.pat) else {
            return;
        };

        let mut collector = UseCollector {
            tcx: cx.tcx,
            target,
            uses: Vec::new(),
        };
        collector.visit_expr(body.value);
        let [use_expr] = collector.uses[..] else {
            return;
        };

        let Node::Expr(conversion) = cx.tcx.parent_hir_node(use_expr.hir_id) else {
            return;
        };
        if !self.is_conversion(cx, typeck, conversion, use_expr) {
            return;
        }
        if !owned.matches_result(cx, typeck.expr_ty(conversion)) {
            return;
        }
        if !is_unconditional(cx, conversion) {
            return;
        }
        if hir_in_external_macro(cx, param.hir_id, param.span) {
            return;
        }

        emit(cx, ident.name, &owned, hir_ty, conversion, use_expr);
    }

    /// Whether `conversion` is `use_expr` being converted to its owned
    /// form — either `use_expr.<method>()` for a recognised method, or
    /// `From::from(use_expr)`. Both branches lean on the caller's
    /// result-type check to confirm the call actually produces the
    /// owned form; the `from` branch additionally requires the callee
    /// to be the `From` trait's `from` (see [`is_from_call`]) so an
    /// unrelated inherent `from` constructor is not matched.
    fn is_conversion<'tcx>(
        &self,
        cx: &LateContext<'tcx>,
        typeck: &ty::TypeckResults<'tcx>,
        conversion: &Expr<'_>,
        use_expr: &Expr<'_>,
    ) -> bool {
        match conversion.kind {
            ExprKind::MethodCall(segment, receiver, _, _) => {
                receiver.hir_id == use_expr.hir_id
                    && self.config.conversion_methods.contains(&segment.ident.name)
            }
            ExprKind::Call(callee, args) => {
                args.len() == 1
                    && args[0].hir_id == use_expr.hir_id
                    && is_from_call(cx, typeck, callee)
            }
            _ => false,
        }
    }
}

/// Collects every expression that resolves to the local binding
/// `target`, descending into nested closure bodies so that a capture
/// counts as a use.
struct UseCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    target: HirId,
    uses: Vec<&'tcx Expr<'tcx>>,
}

impl<'tcx> Visitor<'tcx> for UseCollector<'tcx> {
    type NestedFilter = nested_filter::OnlyBodies;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(QPath::Resolved(None, path)) = expr.kind
            && let Res::Local(hir_id) = path.res
            && hir_id == self.target
        {
            self.uses.push(expr);
        }
        intravisit::walk_expr(self, expr);
    }
}

/// Whether `expr` is reached unconditionally from the enclosing
/// function body — i.e. no `if`/`match`/loop, closure, or
/// short-circuiting logical operator sits between it and the function
/// item. Walking up to the owning item is enough: every conditional
/// construct is an `Expr` node, so any of them on the path proves the
/// expression is conditional. The `?` operator is covered too: it
/// lowers to an `ExprKind::Match`, so the `Match` arm catches it.
fn is_unconditional(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    for (_, node) in cx.tcx.hir_parent_iter(expr.hir_id) {
        match node {
            Node::Expr(ancestor) => match ancestor.kind {
                ExprKind::If(..)
                | ExprKind::Match(..)
                | ExprKind::Loop(..)
                | ExprKind::Closure(..) => return false,
                ExprKind::Binary(op, ..)
                    if matches!(op.node, hir::BinOpKind::And | hir::BinOpKind::Or) =>
                {
                    return false;
                }
                _ => {}
            },
            Node::Item(_) | Node::ImplItem(_) | Node::TraitItem(_) => return true,
            _ => {}
        }
    }
    true
}

/// The owned counterpart of a recognised borrowed parameter type, plus
/// the rendering needed to suggest it.
enum OwnedForm {
    String,
    PathBuf,
    OsString,
    CString,
    /// `Vec<T>`, carrying the pre-rendered `Vec<...>` literal built from
    /// the slice element's source text.
    Vec(String),
}

impl OwnedForm {
    /// The owned type as it should appear in the rewritten signature.
    fn literal(&self) -> String {
        match self {
            OwnedForm::String => "String".to_owned(),
            OwnedForm::PathBuf => "PathBuf".to_owned(),
            OwnedForm::OsString => "OsString".to_owned(),
            OwnedForm::CString => "CString".to_owned(),
            OwnedForm::Vec(literal) => literal.clone(),
        }
    }

    /// Whether the conversion's result type really is this owned form.
    fn matches_result(&self, cx: &LateContext<'_>, result: Ty<'_>) -> bool {
        // `String` is a lang item (`#[lang = "String"]`), not a
        // diagnostic item, so it needs its own lookup; the rest carry a
        // `rustc_diagnostic_item`.
        let diagnostic_name = match self {
            OwnedForm::String => {
                return matches!(
                    result.kind(),
                    ty::Adt(def, _) if Some(def.did()) == cx.tcx.lang_items().string(),
                );
            }
            OwnedForm::PathBuf => "PathBuf",
            OwnedForm::OsString => "OsString",
            OwnedForm::CString => "cstring_type",
            OwnedForm::Vec(_) => "Vec",
        };
        is_diagnostic_ty(cx, result, diagnostic_name)
    }
}

/// The owned counterpart of the borrowed pointee type `pointee`, or
/// `None` when the pointee is not one of the recognised
/// owned-from-borrowed types. `pointee_hir` is the matching written
/// type, used only to recover the slice element's source text.
fn owned_counterpart<'tcx>(
    cx: &LateContext<'tcx>,
    pointee: Ty<'tcx>,
    pointee_hir: &hir::Ty<'_>,
) -> Option<OwnedForm> {
    match pointee.kind() {
        ty::Str => Some(OwnedForm::String),
        ty::Slice(_) => {
            let hir::TyKind::Slice(element) = pointee_hir.kind else {
                return None;
            };
            // Skip rather than render a `Vec<_>` we can't fill in: the
            // element's source text is what the suggestion substitutes
            // for `[T]`, and a `_` placeholder would not compile.
            let element_src = snippet_opt(cx, element.span)?;
            Some(OwnedForm::Vec(format!("Vec<{element_src}>")))
        }
        ty::Adt(def, _) => {
            let did = def.did();
            if is_diagnostic_def(cx, "Path", did) {
                Some(OwnedForm::PathBuf)
            } else if is_diagnostic_def(cx, "OsStr", did) {
                Some(OwnedForm::OsString)
            } else if is_diagnostic_def(cx, "cstr_type", did) {
                Some(OwnedForm::CString)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn is_diagnostic_def(cx: &LateContext<'_>, name: &str, did: DefId) -> bool {
    cx.tcx.is_diagnostic_item(Symbol::intern(name), did)
}

fn is_diagnostic_ty(cx: &LateContext<'_>, ty: Ty<'_>, name: &str) -> bool {
    matches!(ty.kind(), ty::Adt(def, _) if is_diagnostic_def(cx, name, def.did()))
}

/// Whether `callee` is the `From` trait's `from` (`String::from`,
/// `PathBuf::from`, `Vec::from`, a `<Owned as From<_>>::from`, ...). The
/// caller already confirms the call's result type is the owned
/// counterpart, so together this matches exactly `Owned::from(param)`.
/// Requiring the `From` trait keeps the lint from matching an unrelated
/// inherent `Type::from(param)` constructor that merely returns the
/// owned type — dropping such a call in the suggestion would change
/// behaviour.
fn is_from_call<'tcx>(
    cx: &LateContext<'tcx>,
    typeck: &ty::TypeckResults<'tcx>,
    callee: &Expr<'_>,
) -> bool {
    let ExprKind::Path(qpath) = callee.kind else {
        return false;
    };
    let name = match qpath {
        QPath::Resolved(_, path) => path.segments.last().map(|segment| segment.ident.name),
        QPath::TypeRelative(_, segment) => Some(segment.ident.name),
    };
    if name != Some(sym::from) {
        return false;
    }
    let Some(def_id) = typeck.qpath_res(&qpath, callee.hir_id).opt_def_id() else {
        return false;
    };
    is_from_trait_method(cx, def_id)
}

/// Whether `def_id` is `From::from` — either the trait method itself
/// (the usual resolution for `Owned::from(..)`) or a concrete `From`
/// impl's method. Identified by the `from_fn` diagnostic item that the
/// standard library attaches to `From::from`.
fn is_from_trait_method(cx: &LateContext<'_>, def_id: DefId) -> bool {
    let from_fn = Symbol::intern("from_fn");
    if cx.tcx.is_diagnostic_item(from_fn, def_id) {
        return true;
    }
    cx.tcx
        .opt_associated_item(def_id)
        .and_then(|assoc| assoc.trait_item_def_id())
        .is_some_and(|trait_item| cx.tcx.is_diagnostic_item(from_fn, trait_item))
}

fn emit(
    cx: &LateContext<'_>,
    name: Symbol,
    owned: &OwnedForm,
    hir_ty: &hir::Ty<'_>,
    conversion: &Expr<'_>,
    use_expr: &Expr<'_>,
) {
    let owned_literal = owned.literal();
    let message = format!(
        "parameter `{name}` is borrowed but its only use produces an owned `{owned_literal}`",
    );
    let parts = vec![
        (hir_ty.span, owned_literal),
        (
            conversion.span,
            snippet(cx, use_expr.span, name.as_str()).into_owned(),
        ),
    ];
    span_lint_and_then(
        cx,
        NEEDLESS_BORROWED_PARAMETERS,
        hir_ty.span,
        message,
        |diag| {
            diag.multipart_suggestion(
                "take the owned type by value and let callers convert at the call site",
                parts,
                Applicability::MaybeIncorrect,
            );
        },
    );
}
