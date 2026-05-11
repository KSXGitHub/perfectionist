use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::source::indent_of;
use clippy_utils::sym;
use clippy_utils::ty::implements_trait;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_hir::attrs::AttributeKind;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_middle::middle::privacy::Level;
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
    /// The opinion is opt-in: some projects deliberately use exhaustive
    /// error enums to force downstream consumers to handle every new
    /// variant, and binary crates have no SemVer surface to protect.
    /// The lint therefore defaults to `Allow` — enable it per crate
    /// with `#![warn(perfectionist::non_exhaustive_error)]` (or
    /// `deny`) on projects that want it.
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
    Allow,
    "error-shaped type is missing `#[non_exhaustive]`",
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
    /// Visibility threshold for the rule.
    ///
    /// - `"pub"` (default) requires `#[non_exhaustive]` on items
    ///   that are *effectively* reachable from outside the crate
    ///   (declared `pub`, re-exported `pub`, and not buried inside
    ///   a non-`pub` module). A `pub enum FooError` inside a
    ///   non-`pub` module is not flagged because it cannot be
    ///   matched on by any downstream crate.
    /// - `"pub_crate"` additionally requires `#[non_exhaustive]`
    ///   on items literally declared `pub(crate)` (i.e., restricted
    ///   to the crate root). Items declared `pub(in some::module)`
    ///   are not promoted by this mode even if their effective
    ///   reach happens to extend to the crate root.
    /// - `"all"` requires it on every error-shaped item regardless
    ///   of visibility.
    require_for: RequireFor,
    /// Identifier suffixes that mark a type as "an error" purely
    /// by name, without inspecting its trait implementations.
    ///
    /// Setting this option **replaces** the built-in default,
    /// rather than extending it: configuring
    /// `suffixes = ["Failure"]` matches only `*Failure` names, not
    /// `*Error` or `*Failure`. To keep the default suffix alongside
    /// a project-specific one, list it explicitly:
    /// `suffixes = ["Error", "Failure"]`.
    ///
    /// Defaults to `["Error"]`. A type that implements
    /// `std::error::Error` is flagged regardless of suffix.
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
        match self.require_for {
            RequireFor::All => true,
            RequireFor::Pub => is_externally_reachable(tcx, def_id),
            RequireFor::PubCrate => {
                if is_externally_reachable(tcx, def_id) {
                    return true;
                }
                matches!(
                    tcx.visibility(def_id.to_def_id()),
                    ty::Visibility::Restricted(scope) if scope == CRATE_DEF_ID.to_def_id(),
                )
            }
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
        if !self.name_matches(ident.name.as_str()) && !implements_error_trait(cx, local_def_id) {
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
        emit(cx, item, kind_label, ident);
    }
}

/// An item is "externally reachable" when its effective visibility at
/// the `Reexported` level is `Public` — i.e., a downstream crate can
/// see it directly or through the public re-export tree. A `pub` item
/// declared inside a non-`pub` module is *not* externally reachable
/// even though its declared visibility is `Public`, so adding a variant
/// to it can never be a SemVer break.
fn is_externally_reachable(tcx: TyCtxt<'_>, def_id: LocalDefId) -> bool {
    tcx.effective_visibilities(())
        .is_public_at_level(def_id, Level::Reexported)
}

fn implements_error_trait(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
    let Some(error_trait) = cx.tcx.get_diagnostic_item(sym::Error) else {
        return false;
    };
    let ty = cx.tcx.type_of(def_id).instantiate_identity();
    implements_trait(cx, ty, error_trait, &[])
}

/// A struct is "sum-like" when it has exactly one field whose
/// resolved type is an `enum` ADT. `tcx.type_of(..)` already
/// follows transparent type aliases, so `pub struct FooError(pub
/// MyAlias)` where `type MyAlias = MyEnum;` is correctly classified
/// as sum-like — the SemVer-surface concern is identical to a
/// direct `pub struct FooError(pub MyEnum)`.
///
/// Projection types (associated types like `<T as Trait>::Item`)
/// are *not* normalized here, so a sum-like wrapper around an
/// associated type that happens to resolve to an enum is not
/// classified as sum-like. Adding normalization (via
/// `tcx.normalize_erasing_regions`) would close that gap but
/// hasn't been needed in practice — drop a follow-up if a
/// real-world wrapper hits this case.
fn is_sum_like(cx: &LateContext<'_>, data: &hir::VariantData<'_>) -> bool {
    let fields = data.fields();
    if fields.len() != 1 {
        return false;
    }
    let field_ty = cx.tcx.type_of(fields[0].def_id).instantiate_identity();
    matches!(field_ty.kind(), ty::Adt(adt_def, _) if adt_def.is_enum())
}

fn emit(cx: &LateContext<'_>, item: &hir::Item<'_>, kind_label: &str, ident: rustc_span::Ident) {
    let name = ident.name.as_str();
    let message = format!("{kind_label} `{name}` is missing `#[non_exhaustive]`");
    let insert_at = item.span.shrink_to_lo();
    let indent = indent_of(cx, item.span).unwrap_or(0);
    let replacement = format!("#[non_exhaustive]\n{:indent$}", "", indent = indent);
    span_lint_and_then(cx, NON_EXHAUSTIVE_ERROR, ident.span, message, |diag| {
        diag.span_suggestion(
            insert_at,
            "add `#[non_exhaustive]` to keep new variants from being a SemVer break",
            replacement,
            Applicability::MaybeIncorrect,
        );
    });
}
