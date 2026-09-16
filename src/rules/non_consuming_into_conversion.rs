use crate::common::DefaultState;
use crate::field_copy::{
    COPYING_METHODS, Eligible, FieldCopy, borrowed_form, eligible_method, field_copy,
};
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_then;
use core::ops::ControlFlow;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::{Ty, TypeVisitable, TypeVisitor};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags an inherent `into_*` method that does not consume what it
    /// converts: one taking `&self` whose whole body copies a field out
    /// through `clone`, `to_owned`, `to_string`, `to_vec`, `to_path_buf`
    /// or `to_os_string`, and one taking `&self` whose return type
    /// borrows from that receiver.
    ///
    /// A method that moves out of `self` taken by value is left alone,
    /// and so is one returning a `Copy` value: both hand the caller
    /// something of their own, which is what the prefix promises. A
    /// return type carrying a lifetime the *type* already has --
    /// `&'a str` out of a `struct Person<'a>` -- is left alone too, since
    /// that borrow outlives the receiver and does not come from it. So is
    /// a method of a trait impl, since the trait fixes its signature, and
    /// one produced by a macro.
    ///
    /// Test code is left alone; set `exempt_tests` to `false` to
    /// measure it like any other code.
    ///
    /// ### Why is this bad?
    ///
    /// The Rust API Guidelines give `as_`, `to_` and `into_` distinct
    /// meanings, and `into_` is the consuming one: it takes the value and
    /// hands back another the caller owns outright. A caller reads it as
    /// the end of the original's life, and as a conversion that moves
    /// rather than copies. An `into_*` that borrows `self` does neither.
    /// It either pays for a copy the name says was a move, or returns
    /// something still tied to a value the caller was told had been
    /// consumed -- so the original is still alive, and the result cannot
    /// outlive it.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::wrong_self_convention` checks these same prefixes against
    /// the method's *receiver*, so it covers the receiver half of this:
    /// an `into_*` should take `self`. It does not look at the return
    /// type or at what the body costs.
    ///
    /// ### Interaction with sibling rules
    ///
    /// `perfectionist::cloning_as_conversion` and
    /// `perfectionist::borrowing_to_conversion` hold the other two
    /// prefixes to their guideline meanings. Between the three, each
    /// prefix keeps the cost and the ownership its name promises.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn into_name(&self) -> String {
    ///         self.name.clone()
    ///     }
    ///     fn into_name_ref(&self) -> &str {
    ///         &self.name
    ///     }
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn into_name(self) -> String {
    ///         self.name
    ///     }
    /// }
    /// ```
    pub perfectionist::NON_CONSUMING_INTO_CONVERSION,
    Warn,
    "`into_*` method borrows or copies where its prefix promises to consume",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::non_consuming_into_conversion";

/// The first of the two remedies, shared by both halves. A violation is
/// a method that fails to consume *and* carries the `into_` prefix, so
/// dropping either half resolves it: this one drops the failure to
/// consume, the second drops the prefix.
const CONSUME_HELP: &str = "either stop it borrowing: take `self` by value and move the field \
                            out, so the caller gets a value of their own and the original ends \
                            where the name says it does";

/// The second remedy for the borrowing half: the return type may be
/// right and the prefix wrong, and `as_*` is the prefix for handing back
/// a borrow that costs nothing.
const RENAME_HELP: &str = "or stop it being an `into_*`: rename it `as_*`, the prefix for a \
                           conversion that hands back a borrow";

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

pub struct NonConsumingIntoConversion {
    config: Config,
    copying_methods: Vec<Symbol>,
}

impl_lint_pass!(NonConsumingIntoConversion => [NON_CONSUMING_INTO_CONVERSION]);

impl Register for rule::NonConsumingIntoConversion {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[NON_CONSUMING_INTO_CONVERSION]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(NonConsumingIntoConversion {
                config: dylint_linting::config_or_default(CONFIG_KEY),
                copying_methods: COPYING_METHODS
                    .iter()
                    .map(|name| Symbol::intern(name))
                    .collect(),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for NonConsumingIntoConversion {
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
        if !method.as_str().starts_with("into_") {
            return;
        }
        if self.config.exempt_tests && item_in_test_code(cx, def_id) {
            return;
        }
        let output = cx
            .tcx
            .fn_sig(def_id)
            .instantiate_identity()
            .skip_binder()
            .output();
        if borrows_from_receiver(output) {
            span_lint_and_then(
                cx,
                NON_CONSUMING_INTO_CONVERSION,
                def_span,
                format!(
                    "`{method}` returns a borrow of `self`, but `into_` promises to consume it",
                ),
                |diag| {
                    diag.help(CONSUME_HELP);
                    diag.help(RENAME_HELP);
                },
            );
            return;
        }
        let Some(FieldCopy {
            field, field_ty, ..
        }) = field_copy(cx, kind, decl, body, def_id, &self.copying_methods)
        else {
            return;
        };
        span_lint_and_then(
            cx,
            NON_CONSUMING_INTO_CONVERSION,
            def_span,
            format!("`{method}` copies `self.{field}`, but `into_` promises to consume it"),
            |diag| {
                diag.help(CONSUME_HELP);
                diag.help(format!(
                    "or stop it being an `into_*`: return `{}` and rename it `as_*`, if the \
                     caller only needs to read it",
                    borrowed_form(cx, field_ty),
                ));
            },
        );
    }
}

/// Whether `ty` carries a lifetime this method itself introduced, which
/// for a `&self` method is the borrow of the receiver.
///
/// The distinction that matters is where the lifetime comes from. A
/// method's own lifetimes -- the elided one behind `&self` included --
/// are bound by the signature's binder, so they appear as `ReBound` once
/// it is stripped. A lifetime the *type* carries, as in
/// `struct Person<'a>`, is a parameter of the impl, so it survives
/// `instantiate_identity` as `ReEarlyParam` and is not one of these. That
/// second kind outlives the receiver, so returning it consumes nothing
/// the caller still holds.
fn borrows_from_receiver<'tcx>(ty: Ty<'tcx>) -> bool {
    struct Finder;
    impl<'tcx> TypeVisitor<rustc_middle::ty::TyCtxt<'tcx>> for Finder {
        type Result = ControlFlow<()>;

        fn visit_region(&mut self, region: rustc_middle::ty::Region<'tcx>) -> Self::Result {
            if matches!(region.kind(), rustc_middle::ty::ReBound(..)) {
                return ControlFlow::Break(());
            }
            ControlFlow::Continue(())
        }
    }
    ty.visit_with(&mut Finder).is_break()
}
