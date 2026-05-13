use clippy_utils::diagnostics::span_lint_and_sugg;
use clippy_utils::sugg::Sugg;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::sym;

use crate::common::hir_in_external_macro;

declare_tool_lint! {
    /// ### What it does
    /// Flags `value.clone()` where `value` is an `Arc<T>` or `Rc<T>`,
    /// and suggests rewriting it as the qualified `Arc::clone(...)` /
    /// `Rc::clone(...)` form. For a receiver of type `Arc<T>` /
    /// `Rc<T>`, the rewrite is `Arc::clone(&value)`; for a receiver
    /// already typed as `&Arc<T>` / `&Rc<T>`, the leading `&` is
    /// dropped (`Arc::clone(value)`) so the result doesn't form a
    /// stutter-borrow `&&Arc<T>`.
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
    /// call compiles; the practice forbidden here is the *method-call
    /// dispatch* (`value.clone()`), where the impl Rust picks
    /// depends on the receiver's type. The accepted qualified forms
    /// — bare (`Arc::clone(&value)`), turbofish
    /// (`Arc::<T>::clone(&value)`), and UFCS
    /// (`<Arc<T> as Clone>::clone(&value)`) — all name the impl at
    /// the call site, so the cost is visible to the reader. Two
    /// reasons to prefer them:
    ///
    /// - **Explicit cost.** `Arc::clone` is `O(1)` reference-count
    ///   bump regardless of what `T` is; `T::clone` may be an
    ///   arbitrarily expensive deep copy. If the binding's type
    ///   later changes from `Arc<Vec<u8>>` to `&Vec<u8>`, the
    ///   method-call form `value.clone()` silently switches to
    ///   `<Vec<u8> as Clone>::clone` and starts allocating; the
    ///   qualified form does not type check and fails loudly.
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
    "calling `.clone()` on an `Arc<T>` or `Rc<T>`; prefer the qualified `Arc::clone` / `Rc::clone` form",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::arc_rc_clone";

/// Configuration is reserved for future knobs; the lint currently
/// has no options. The empty struct still exists so that a stray
/// `[perfectionist::arc_rc_clone]` table in `dylint.toml`
/// deserialises rather than producing a confusing parse error.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
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
    if !crate::common::is_enabled("arc_rc_clone", true) {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(ArcRcClone::new()));
}

impl<'tcx> LateLintPass<'tcx> for ArcRcClone {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        let hir::ExprKind::MethodCall(method_segment, receiver, [], _) = expr.kind else {
            return;
        };
        if method_segment.ident.name != sym::clone {
            return;
        }
        if hir_in_external_macro(cx, expr.hir_id, expr.span) {
            return;
        }
        // Match on the method call's *result* type rather than the
        // receiver: that's the type a refcount-bumping `.clone()`
        // actually produces, so it filters out shapes where the
        // blanket `<&T as Clone>::clone` wins method probe and the
        // call returns a reference instead. With a receiver of type
        // `&&Arc<T>`, for instance, `.clone()` returns `&Arc<T>`
        // (a pointer copy, not a refcount bump); rewriting that to
        // `Arc::clone(...)` would change the expression's type.
        let ty::Adt(adt, _) = cx.typeck_results().expr_ty(expr).kind() else {
            return;
        };
        let kind = match cx.tcx.get_diagnostic_name(adt.did()) {
            Some(sym::Arc) => "Arc",
            Some(sym::Rc) => "Rc",
            _ => return,
        };
        // The autofix renders the bare type name (`Arc` / `Rc`),
        // which assumes the user has the type in scope. Code that
        // uses only the full path — e.g. `std::sync::Arc::new(v).clone()`
        // without `use std::sync::Arc` — would not compile after the
        // rewrite, so the applicability is `MaybeIncorrect` rather
        // than `MachineApplicable`: the suggestion still surfaces to
        // the user, but `cargo fix` won't apply it silently. Same
        // trade-off `clippy::clone_on_ref_ptr` carries, with a
        // tighter applicability label.
        let mut applicability = Applicability::MaybeIncorrect;
        let receiver_sugg =
            Sugg::hir_with_applicability(cx, receiver, "_", &mut applicability).maybe_paren();
        // For receivers that are already `&Arc<T>` / `&Rc<T>` (or
        // `&mut`), drop the leading `&` so `arc_ref.clone()` rewrites
        // to `Arc::clone(arc_ref)` rather than the harder-to-read
        // `Arc::clone(&arc_ref)` (which would form `&&Arc<T>`).
        let leading_amp = if matches!(cx.typeck_results().expr_ty(receiver).kind(), ty::Ref(..)) {
            ""
        } else {
            "&"
        };
        span_lint_and_sugg(
            cx,
            ARC_RC_CLONE,
            expr.span,
            format!("calling `.clone()` on an `{kind}<T>`"),
            "use the qualified form to make the cheap refcount bump explicit",
            format!("{kind}::clone({leading_amp}{receiver_sugg})"),
            applicability,
        );
    }
}
