use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::source::indent_of;
use clippy_utils::sym;
use clippy_utils::ty::implements_trait;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_hir::attrs::AttributeKind;
use rustc_hir::def::{DefKind, Res};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::ty::{self, TyCtxt};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::def_id::{CRATE_DEF_ID, LocalDefId};

declare_tool_lint! {
    /// ### What it does
    /// Flags publicly-exposed error enums that lack a `#[non_exhaustive]`
    /// attribute. An enum is treated as an error enum when its name ends
    /// in `Error` (configurable) or it implements `std::error::Error`.
    /// Publicly-exposed sum-like structs (a single field whose type is
    /// itself an enum) follow the same rule.
    ///
    /// "Publicly-exposed" defaults to `pub` items; `pub(crate)` and the
    /// whole-crate "every item" sweep are configurable.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. Adding
    /// a variant to an error enum is one of the most common reasons to
    /// publish a new minor version of an error-producing library, and
    /// `#[non_exhaustive]` is the standard way to make that addition
    /// not a SemVer break for downstream pattern matches. Applying it
    /// up front means future variants land without a coordinated major
    /// release across the dependents that exhaustively match on the
    /// enum.
    ///
    /// ### Example
    /// ```rust,ignore
    /// #[derive(Debug)]
    /// pub enum RuntimeError {
    ///     SerializationFailure,
    /// }
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// #[derive(Debug)]
    /// #[non_exhaustive]
    /// pub enum RuntimeError {
    ///     SerializationFailure,
    /// }
    /// ```
    pub perfectionist::NON_EXHAUSTIVE_ERROR,
    Warn,
    "publicly-exposed error type is missing `#[non_exhaustive]`",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::non_exhaustive_error";

#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum RequireFor {
    #[default]
    Pub,
    PubCrate,
    All,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
struct Config {
    /// Visibility threshold for the rule. `"pub"` (default) only
    /// requires `#[non_exhaustive]` on fully-public items;
    /// `"pub_crate"` additionally requires it on `pub(crate)` items;
    /// `"all"` requires it on every error-shaped item regardless of
    /// visibility.
    require_for: RequireFor,
    /// Identifier suffixes that mark a type as "an error" purely
    /// by name, without inspecting its trait implementations.
    /// Defaults to `["Error"]`; extend with project conventions like
    /// `"Failure"`. A type that implements `std::error::Error` is
    /// flagged regardless of suffix.
    suffixes: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            require_for: RequireFor::default(),
            suffixes: vec!["Error".to_owned()],
        }
    }
}

pub struct NonExhaustiveError {
    require_for: RequireFor,
    suffixes: Vec<String>,
}

impl NonExhaustiveError {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            require_for: config.require_for,
            suffixes: config.suffixes,
        }
    }

    fn visibility_qualifies(&self, tcx: TyCtxt<'_>, def_id: LocalDefId) -> bool {
        let vis = tcx.visibility(def_id.to_def_id());
        match self.require_for {
            RequireFor::All => true,
            RequireFor::PubCrate => match vis {
                ty::Visibility::Public => true,
                ty::Visibility::Restricted(scope) => scope == CRATE_DEF_ID.to_def_id(),
            },
            RequireFor::Pub => matches!(vis, ty::Visibility::Public),
        }
    }

    fn name_matches(&self, name: &str) -> bool {
        self.suffixes
            .iter()
            .any(|suffix| name.ends_with(suffix.as_str()))
    }
}

impl_lint_pass!(NonExhaustiveError => [NON_EXHAUSTIVE_ERROR]);

/// Register this rule's lint declaration. Paired with [`register_pass`];
/// see the module-level convention documented in `register_lints`.
pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[NON_EXHAUSTIVE_ERROR]);
}

/// Install this rule's late pass.
pub fn register_pass(lint_store: &mut LintStore) {
    lint_store.register_late_pass(|_| Box::new(NonExhaustiveError::new()));
}

impl<'tcx> LateLintPass<'tcx> for NonExhaustiveError {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        let (ident, kind_label) = match item.kind {
            hir::ItemKind::Enum(ident, _, _) => (ident, "enum"),
            hir::ItemKind::Struct(ident, _, ref data) => {
                if !is_sum_like(cx, data) {
                    return;
                }
                (ident, "struct")
            }
            _ => return,
        };
        let local_def_id = item.owner_id.def_id;
        if !self.visibility_qualifies(cx.tcx, local_def_id) {
            return;
        }
        let name = ident.name.as_str();
        if !self.name_matches(name) && !implements_error_trait(cx, local_def_id) {
            return;
        }
        let attrs = cx.tcx.hir_attrs(item.hir_id());
        if attrs.iter().any(|attr| {
            matches!(
                attr,
                hir::Attribute::Parsed(AttributeKind::NonExhaustive(_)),
            )
        }) {
            return;
        }
        emit(cx, item, kind_label, name);
    }
}

fn implements_error_trait(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
    let Some(error_trait) = cx.tcx.get_diagnostic_item(sym::Error) else {
        return false;
    };
    let ty = cx.tcx.type_of(def_id).instantiate_identity();
    implements_trait(cx, ty, error_trait, &[])
}

/// A struct is "sum-like" when it has exactly one field and that
/// field's type resolves to an `enum`. The rationale matches the
/// planning file: such a struct is a newtype around an enum, so its
/// SemVer surface inherits the enum's variant-addition concern.
fn is_sum_like(cx: &LateContext<'_>, data: &hir::VariantData<'_>) -> bool {
    let fields = data.fields();
    if fields.len() != 1 {
        return false;
    }
    let field_ty = fields[0].ty;
    let hir::TyKind::Path(qpath) = field_ty.kind else {
        return false;
    };
    matches!(
        cx.qpath_res(&qpath, field_ty.hir_id),
        Res::Def(DefKind::Enum, _),
    )
}

fn emit(cx: &LateContext<'_>, item: &hir::Item<'_>, kind_label: &str, name: &str) {
    let message = format!("public {kind_label} `{name}` is missing `#[non_exhaustive]`");
    let insert_at = item.span.shrink_to_lo();
    let indent = indent_of(cx, item.span).unwrap_or(0);
    let replacement = format!("#[non_exhaustive]\n{:indent$}", "", indent = indent);
    span_lint_and_then(cx, NON_EXHAUSTIVE_ERROR, item.span, message, |diag| {
        diag.span_suggestion(
            insert_at,
            "add `#[non_exhaustive]` to keep new variants from being a SemVer break",
            replacement,
            Applicability::MaybeIncorrect,
        );
    });
}
