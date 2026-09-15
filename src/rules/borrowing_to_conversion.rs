use crate::common::DefaultState;
use crate::field_copy::{Eligible, eligible_method};
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::{self, Ty};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags an inherent `to_*` method taking `&self` that returns a
    /// reference -- `&T`, or an `Option<&T>` -- and asks for the `as_*`
    /// prefix instead.
    ///
    /// A method of a trait impl is left alone, since the trait fixes its
    /// signature, and so is a method produced by a macro.
    ///
    /// Test code is left alone; set `exempt_tests` to `false` to
    /// measure it like any other code.
    ///
    /// ### Why is this bad?
    ///
    /// The Rust API Guidelines give `as_`, `to_` and `into_` distinct
    /// meanings. `to_` is the costly one, borrowed to owned, and `as_` is
    /// the free one, borrowed to borrowed. A `to_*` that hands back a
    /// reference has done the free conversion under the costly name, so a
    /// caller who could have used it freely avoids it, and one reading
    /// the signature has to look twice to see that nothing was allocated.
    /// The name is the only thing wrong, and renaming is the whole fix.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::wrong_self_convention` checks these same prefixes against
    /// the method's *receiver* -- whether `to_*` takes `&self`. It does
    /// not look at the return type, so a `to_*` that takes `&self` and
    /// returns a borrow satisfies it.
    ///
    /// ### Interaction with sibling rules
    ///
    /// `perfectionist::cloning_as_conversion` is this rule's mirror: it
    /// flags an `as_*` that copies, where this flags a `to_*` that does
    /// not. Between them the two prefixes keep their guideline meanings,
    /// and each rule's fix is the other's prefix.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn to_name(&self) -> &str {
    ///         &self.name
    ///     }
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn as_name(&self) -> &str {
    ///         &self.name
    ///     }
    /// }
    /// ```
    pub perfectionist::BORROWING_TO_CONVERSION,
    Warn,
    "`to_*` method returns a reference where its prefix promises an owned value",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::borrowing_to_conversion";

/// The second reading of the same signature: the name may be right and
/// the return type wrong, which is the other way to make them agree.
const OWNED_HELP: &str = "or return the owned value the name promises, where a caller really does \
                          need one of its own";

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Whether test code is left alone: methods inside a `#[cfg(test)]`
    /// module or an integration-test or benchmark target. Defaults to
    /// `true`.
    exempt_tests: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { exempt_tests: true }
    }
}

pub struct BorrowingToConversion {
    config: Config,
}

impl_lint_pass!(BorrowingToConversion => [BORROWING_TO_CONVERSION]);

impl Register for rule::BorrowingToConversion {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[BORROWING_TO_CONVERSION]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(BorrowingToConversion {
                config: dylint_linting::config_or_default(CONFIG_KEY),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for BorrowingToConversion {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _span: Span,
        def_id: LocalDefId,
    ) {
        let Some(Eligible { method, def_span }) = eligible_method(cx, kind, decl, body, def_id)
        else {
            return;
        };
        if !method.as_str().starts_with("to_") {
            return;
        }
        let output = cx
            .tcx
            .fn_sig(def_id)
            .instantiate_identity()
            .skip_binder()
            .output();
        if !returns_a_borrow(cx, output) {
            return;
        }
        if self.config.exempt_tests && item_in_test_code(cx, def_id) {
            return;
        }
        let suggested = method.as_str().replacen("to_", "as_", 1);
        span_lint_and_then(
            cx,
            BORROWING_TO_CONVERSION,
            def_span,
            format!("`{method}` returns a borrow, but `to_` promises an owned value"),
            |diag| {
                diag.help(format!(
                    "rename it `{suggested}`, the prefix for a conversion that costs nothing",
                ));
                diag.help(OWNED_HELP);
            },
        );
    }
}

/// Whether `ty` hands the caller a borrow rather than a value of their
/// own: a reference, or an `Option` of one. An `Option<&T>` is as free as
/// the `&T` inside it, so the prefix misleads either way.
fn returns_a_borrow<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    if ty.is_ref() {
        return true;
    }
    let ty::Adt(adt, args) = ty.kind() else {
        return false;
    };
    if !cx
        .tcx
        .is_diagnostic_item(Symbol::intern("Option"), adt.did())
    {
        return false;
    }
    args.types().next().is_some_and(Ty::is_ref)
}
