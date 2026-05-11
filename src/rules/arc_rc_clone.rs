use clippy_utils::diagnostics::span_lint_and_sugg;
use clippy_utils::sugg::Sugg;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::sym;

declare_tool_lint! {
    /// ### What it does
    /// Flags `value.clone()` where `value` is an `Arc<T>` or `Rc<T>`,
    /// and suggests rewriting it as `Arc::clone(&value)` /
    /// `Rc::clone(&value)`.
    ///
    /// The qualified form is accepted in every shape: the bare
    /// `Arc::clone(...)`, the turbofish-typed `Arc::<T>::clone(...)`,
    /// and the UFCS `<Arc<T> as Clone>::clone(...)` are all left
    /// untouched. The lint targets only the method-call shape, which
    /// reads as a generic `Clone` call rather than the cheap refcount
    /// bump it actually is.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue.
    /// `Arc<T>` and `Rc<T>` implement `Clone` precisely so the method
    /// call compiles; the practice forbidden here is calling it
    /// through the `Clone` trait by name. Two reasons to prefer the
    /// qualified form:
    ///
    /// - **Explicit cost.** `Arc::clone` is `O(1)` reference-count
    ///   bump regardless of what `T` is; `T::clone` may be an
    ///   arbitrarily expensive deep copy. If the binding's type later
    ///   changes from `Arc<T>` to `&T`, the method-call form silently
    ///   switches to the latter — the qualified form does not type-
    ///   check and fails loudly.
    /// - **Reader signal.** `Arc::clone(&handle)` reads as "share a
    ///   handle"; `handle.clone()` reads as a generic `Clone`
    ///   invocation whose cost is unknown without checking the
    ///   binding's type.
    ///
    /// ### Example
    /// ```rust,ignore
    /// fn spawn_worker(state: std::sync::Arc<State>) {
    ///     let copy = state.clone();
    ///     thread::spawn(move || work(copy));
    /// }
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// fn spawn_worker(state: std::sync::Arc<State>) {
    ///     let copy = std::sync::Arc::clone(&state);
    ///     thread::spawn(move || work(copy));
    /// }
    /// ```
    pub perfectionist::ARC_RC_CLONE,
    Warn,
    "calling `.clone()` on an `Arc<T>` or `Rc<T>`; prefer the qualified `Arc::clone(&x)` form",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::arc_rc_clone";

/// Configuration is reserved for future knobs; the lint currently
/// has no options. The empty struct still exists so that a stray
/// `[perfectionist::arc_rc_clone]` table in `dylint.toml`
/// deserialises rather than producing a confusing parse error.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {}

pub struct ArcRcClone;

impl ArcRcClone {
    fn new() -> Self {
        let _config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self
    }
}

impl_lint_pass!(ArcRcClone => [ARC_RC_CLONE]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[ARC_RC_CLONE]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    lint_store.register_late_pass(|_| Box::new(ArcRcClone::new()));
}

impl<'tcx> LateLintPass<'tcx> for ArcRcClone {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        // Expansion-originated `.clone()` calls (e.g. inside a
        // third-party macro) are out of the user's hands; let them
        // pass.
        if expr.span.from_expansion() {
            return;
        }
        let hir::ExprKind::MethodCall(method_segment, receiver, [], _) = expr.kind else {
            return;
        };
        if method_segment.ident.name != sym::clone {
            return;
        }
        // Peeling references covers both the direct `value.clone()`
        // shape (receiver type `Arc<T>`) and the deref-then-clone
        // shape (receiver type `&Arc<T>`); the rule's accepted
        // suggested fix is the same `Arc::clone(&...)` form for both.
        let receiver_ty = cx.typeck_results().expr_ty(receiver).peel_refs();
        let ty::Adt(adt, _) = receiver_ty.kind() else {
            return;
        };
        let kind = match cx.tcx.get_diagnostic_name(adt.did()) {
            Some(sym::Arc) => "Arc",
            Some(sym::Rc) => "Rc",
            _ => return,
        };
        let mut applicability = Applicability::MachineApplicable;
        let receiver_sugg =
            Sugg::hir_with_applicability(cx, receiver, "_", &mut applicability).maybe_paren();
        span_lint_and_sugg(
            cx,
            ARC_RC_CLONE,
            expr.span,
            format!("using `.clone()` on an `{kind}<T>`"),
            "use the qualified form to make the cheap refcount bump explicit",
            format!("{kind}::clone(&{receiver_sugg})"),
            applicability,
        );
    }
}
