use crate::common::DefaultState;
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::ty::is_copy;
use rustc_hir as hir;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Expr, ExprKind, ImplicitSelfKind, QPath};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::print::ForceTrimmedGuard;
use rustc_middle::ty::{self, AssocContainer, Ty, TypeckResults};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{Span, Symbol, kw};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags a getter — an inherent method taking `&self` whose whole
    /// body is one field of `self` copied out through `clone`,
    /// `to_owned`, `to_string`, `to_vec`, `to_path_buf`, or
    /// `to_os_string` — and asks for the borrowed form instead: `&str`
    /// for a `String` field, `&Path` for a `PathBuf`, `&OsStr` for an
    /// `OsString`, `&[T]` for a `Vec<T>`, `Option<&T>` for an
    /// `Option<T>`, `&T` otherwise.
    ///
    /// The call has to reproduce the field's own type for a borrow to
    /// serve in its place. So a `Copy` field is left alone — returning
    /// it by value is the borrowed form's equal — and so is a call that
    /// renders the field rather than copying it, such as `to_string` on
    /// a numeric field, where no borrow of the field is a `String`.
    ///
    /// A method of a trait impl is left alone, since the trait fixes
    /// its signature, and so is a method produced by a macro.
    ///
    /// Test code is measured like any other code; set
    /// `exempt_tests` to leave it alone.
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
    /// `clippy::clone_on_copy` is what catches the `.clone()` on a
    /// `Copy` field that this rule leaves alone. No Clippy lint looks at
    /// what a getter returns.
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

/// How to tell the fix did not work, in the shape the sibling rules
/// use. The payoff this rule claims is that most callers
/// only read the value, so a call site that copies the borrow straight
/// back is what says the borrow bought nothing.
const COPY_BACK_HELP: &str = "if every call site copies the borrow straight back, the copy moved \
                              rather than went away: the callers did want ownership, and the owned \
                              return was right";

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
    exempt_tests: bool,
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
        let hir_id = cx.tcx.local_def_id_to_hir_id(def_id);
        if def_span.from_expansion()
            || clippy_utils::is_from_proc_macro(cx, &(&kind, body, hir_id, def_span))
        {
            return;
        }
        // A trait fixes the signature of its methods.
        if let Some(assoc) = cx.tcx.opt_associated_item(def_id.to_def_id())
            && !matches!(assoc.container, AssocContainer::InherentImpl)
        {
            return;
        }
        let typeck = cx.tcx.typeck(def_id);
        let Some((field, field_ty)) = self.copied_field(cx, typeck, body.value) else {
            return;
        };
        if self.config.exempt_tests && item_in_test_code(cx, def_id) {
            return;
        }
        let getter = ident.name;
        span_lint_and_then(
            cx,
            CLONING_GETTER,
            def_span,
            format!("getter `{getter}` returns an owned copy of `self.{field}`"),
            |diag| {
                diag.help(format!(
                    "return `{}` and let a caller that needs ownership copy at the call site",
                    borrowed_form(cx, field_ty),
                ));
                diag.help(COPY_BACK_HELP);
            },
        );
    }
}

impl CloningGetter {
    /// The field the body copies out, when the body is exactly
    /// `self.<field>.<copying method>()`, possibly wrapped in a block,
    /// and a borrow of that field could have served in the copy's place.
    fn copied_field<'tcx>(
        &self,
        cx: &LateContext<'tcx>,
        typeck: &TypeckResults<'tcx>,
        body: &'tcx Expr<'tcx>,
    ) -> Option<(Symbol, Ty<'tcx>)> {
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
        if segment.ident.name != kw::SelfLower {
            return None;
        }
        let field_ty = typeck.expr_ty(receiver);
        // The call has to reproduce the field's own type for a borrow of
        // the field to serve in its place. `self.count.to_string()`
        // renders a `u32`, and no borrow of `self.count` is a `String`.
        if typeck.expr_ty(expr) != field_ty {
            return None;
        }
        // A `Copy` field returned by value is the borrowed form's equal.
        // A shared reference is itself `Copy`, so this also leaves alone a
        // field that is already a borrow, where the call copies nothing.
        if is_copy(cx, field_ty) {
            return None;
        }
        Some((field.name, field_ty))
    }
}

/// The borrowed form a caller could take in place of the field's owned
/// type: `&str` for a `String`, `&Path` for a `PathBuf`, `&OsStr` for an
/// `OsString`, `&[T]` for a `Vec<T>`, the inner type's own borrowed form
/// under an `Option`, and `&T` for anything else.
fn borrowed_form<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> String {
    // Print paths trimmed to their final segment, so the help reads
    // `&[String]` rather than `&[std::string::String]`. The guard is what
    // `with_forced_trimmed_paths!` expands to, held here for the whole
    // function rather than wrapped around each `format!`.
    let _trimmed = ForceTrimmedGuard::new();
    let ty::Adt(adt, args) = ty.kind() else {
        return format!("&{ty}");
    };
    let did = adt.did();
    // `String` is a lang item (`#[lang = "String"]`), not a diagnostic
    // item, so it needs its own lookup; the rest carry a
    // `rustc_diagnostic_item`.
    if Some(did) == cx.tcx.lang_items().string() {
        return "&str".to_owned();
    }
    let is = |name: &str| cx.tcx.is_diagnostic_item(Symbol::intern(name), did);
    if is("PathBuf") {
        return "&Path".to_owned();
    }
    if is("OsString") {
        return "&OsStr".to_owned();
    }
    let Some(inner) = args.types().next() else {
        return format!("&{ty}");
    };
    if is("Vec") {
        return format!("&[{inner}]");
    }
    if is("Option") {
        return format!("Option<{}>", borrowed_form(cx, inner));
    }
    format!("&{ty}")
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
