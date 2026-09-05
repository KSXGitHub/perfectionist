use crate::common::DefaultState;
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Expr, ExprKind, ImplicitSelfKind, QPath};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::AssocContainer;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol, kw};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a getter — an inherent method taking `&self` whose whole
    /// body is one field of `self` copied out through `clone`,
    /// `to_owned`, `to_string`, `to_vec`, `to_path_buf`, or
    /// `to_os_string` — and asks for the borrowed form instead: `&str`
    /// for a `String` field, `&Path` for a `PathBuf`, `&[T]` for a
    /// `Vec<T>`, `Option<&T>` for an `Option<T>`, `&T` otherwise.
    ///
    /// A method of a trait impl is left alone, since the trait fixes
    /// its signature, and so is a method produced by a macro.
    ///
    /// Test code is measured like any other code; set
    /// `test_code_exception` to leave it alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A
    /// getter that clones decides for every caller that they wanted an
    /// owned copy, and most did not: they compare, print, or pass the
    /// value on. The borrowed form serves every caller, costs nothing,
    /// and leaves the one caller that does need ownership to say so
    /// with a `.to_owned()` at the call site, where the reader can see
    /// the copy. It also stops the getter from advertising the field's
    /// representation: a `&str` getter can later be backed by a
    /// `Box<str>` or an interned symbol without a caller changing.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::clone_on_copy` catches a `.clone()` on a `Copy` field,
    /// which this rule does not flag: returning a `u32` by value is the
    /// borrowed form's equal. No Clippy lint looks at what a getter
    /// returns.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn first_name(&self) -> String {
    ///         self.first_name.clone()
    ///     }
    ///     fn middle_name(&self) -> Option<String> {
    ///         self.middle_name.clone()
    ///     }
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// impl Person {
    ///     fn first_name(&self) -> &str {
    ///         &self.first_name
    ///     }
    ///     fn middle_name(&self) -> Option<&str> {
    ///         self.middle_name.as_deref()
    ///     }
    /// }
    /// ```
    pub perfectionist::CLONING_GETTER,
    Warn,
    "getter returns an owned copy of a field where a borrow would serve",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::cloning_getter";

/// The methods that turn a borrowed field into its owned form.
const COPYING_METHODS: &[&str] = &[
    "clone",
    "to_owned",
    "to_string",
    "to_vec",
    "to_path_buf",
    "to_os_string",
];

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Whether test code is left alone: getters inside a `#[cfg(test)]`
    /// module or an integration-test or benchmark target. Defaults to
    /// `false`.
    test_code_exception: bool,
}

pub struct CloningGetter {
    config: Config,
    copying_methods: Vec<Symbol>,
}

impl_lint_pass!(CloningGetter => [CLONING_GETTER]);

impl Register for rule::CloningGetter {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[CLONING_GETTER]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(CloningGetter {
                config: dylint_linting::config_or_default(CONFIG_KEY),
                copying_methods: COPYING_METHODS
                    .iter()
                    .map(|name| Symbol::intern(name))
                    .collect(),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for CloningGetter {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        _span: Span,
        def_id: LocalDefId,
    ) {
        let FnKind::Method(ident, _) = kind else {
            return;
        };
        if !matches!(decl.implicit_self(), ImplicitSelfKind::RefImm) || decl.inputs.len() != 1 {
            return;
        }
        let def_span = cx.tcx.def_span(def_id);
        if def_span.from_expansion() {
            return;
        }
        // A trait fixes the signature of its methods.
        if let Some(assoc) = cx.tcx.opt_associated_item(def_id.to_def_id())
            && !matches!(assoc.container, AssocContainer::InherentImpl)
        {
            return;
        }
        let Some(field) = self.copied_field(body.value) else {
            return;
        };
        if self.config.test_code_exception && item_in_test_code(cx, def_id) {
            return;
        }
        let getter = ident.name;
        span_lint_and_help(
            cx,
            CLONING_GETTER,
            def_span,
            format!("getter `{getter}` returns an owned copy of `self.{field}`"),
            None,
            "return a borrow (`&str`, `&Path`, `&[T]`, `Option<&T>`, `&T`) and let a caller that needs ownership copy at the call site",
        );
    }
}

impl CloningGetter {
    /// The field the body copies out, when the body is exactly
    /// `self.<field>.<copying method>()`, possibly wrapped in a block.
    fn copied_field(&self, body: &Expr<'_>) -> Option<Symbol> {
        let expr = unwrap_block(body);
        let ExprKind::MethodCall(segment, receiver, [], _) = expr.kind else {
            return None;
        };
        if !self.copying_methods.contains(&segment.ident.name) {
            return None;
        }
        let ExprKind::Field(base, field) = receiver.kind else {
            return None;
        };
        let ExprKind::Path(QPath::Resolved(None, path)) = base.kind else {
            return None;
        };
        let [segment] = path.segments else {
            return None;
        };
        (segment.ident.name == kw::SelfLower).then_some(field.name)
    }
}

/// The expression a body of nested `{ }` blocks with no statements
/// comes down to.
fn unwrap_block<'a>(mut expr: &'a Expr<'a>) -> &'a Expr<'a> {
    while let ExprKind::Block(block, None) = expr.kind
        && block.stmts.is_empty()
        && let Some(inner) = block.expr
    {
        expr = inner;
    }
    expr
}
