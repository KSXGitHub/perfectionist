use crate::common::{DefaultState, resolve_string_set, resolved_state};
use crate::rule_index::{ExhaustiveErrorEnumsRule, RuleRegistration};
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
use std::collections::BTreeSet;

declare_tool_lint! {
    /// ### What it does
    ///
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
    ///
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
    /// The rule is therefore inactive by default — enable it per
    /// crate by adding to `dylint.toml`:
    ///
    /// ```toml
    /// [perfectionist]
    /// enable = ["exhaustive_error_enums"]
    /// ```
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::exhaustive_enums` and `clippy::exhaustive_structs`
    /// (`restriction`, off by default) require `#[non_exhaustive]` on
    /// *every* exported enum / struct. This rule scopes that same
    /// requirement to **error types** — those whose name ends in
    /// `Error` or that implement `std::error::Error` — because adding
    /// a variant to an error type downstream code matches on is the
    /// breaking change `#[non_exhaustive]` exists to absorb, while
    /// demanding it on all exported types is usually too broad. Enable
    /// the Clippy lints instead if you do want that blanket policy.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// #[derive(Debug)]
    /// pub enum RuntimeError {
    ///     SerializationFailure,
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// #[derive(Debug)]
    /// #[non_exhaustive]
    /// pub enum RuntimeError {
    ///     SerializationFailure,
    /// }
    /// ```
    pub perfectionist::EXHAUSTIVE_ERROR_ENUMS,
    Warn,
    "error-shaped type is missing `#[non_exhaustive]`",
    report_in_external_macro: false
}

/// Off by default — enable it in `dylint.toml` via the crate-wide
/// `[perfectionist] enable = ["exhaustive_error_enums"]` (or the
/// `[[perfectionist.enable]]` array-of-tables form). Read by
/// [`RuleRegistration::register_pass`] below; gen-docs picks the
/// constant up via syn to render the rule's default state.
pub(crate) const DEFAULT_STATE: DefaultState = DefaultState::Inactive;

const CONFIG_KEY: &str = "perfectionist::exhaustive_error_enums";

#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum RequireFor {
    /// Require `#[non_exhaustive]` on items that are *effectively*
    /// reachable from outside the crate (declared `pub`, re-exported
    /// `pub`, and not buried inside a non-`pub` module). A
    /// `pub enum FooError` inside a non-`pub` module is not flagged
    /// because it cannot be matched on by any downstream crate.
    #[default]
    Pub,
    /// In addition to the `Pub` case, require `#[non_exhaustive]`
    /// on items literally declared `pub(crate)` (i.e., restricted
    /// to the crate root). Items declared `pub(in some::module)`
    /// are not promoted by this mode even if their effective reach
    /// happens to extend to the crate root.
    PubCrate,
    /// Require `#[non_exhaustive]` on every error-shaped item
    /// regardless of visibility.
    All,
}

/// Default identifier suffixes that mark a type as "an error"
/// purely by name. A type that implements `std::error::Error` is
/// flagged regardless of suffix.
const DEFAULT_SUFFIXES: &[&str] = &["Error"];

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Visibility threshold for the rule.
    require_for: RequireFor,
    /// Additional identifier suffixes that mark a type as "an
    /// error" purely by name, without inspecting its trait
    /// implementations. Merged with the built-in defaults
    /// (`["Error"]`); empty by default. List project-specific
    /// vocabulary here (`Failure`, `Fault`, ...) without having to
    /// re-state the standard suffix.
    extra_suffixes: Vec<String>,
    /// Identifier suffixes to drop from the by-name match set,
    /// even if they appear in the built-in defaults or in
    /// `extra_suffixes`.
    /// Empty by default; checked after the merge with the
    /// built-ins, so this knob always wins. Use it when a project
    /// deliberately does not want the `Error` suffix to trigger
    /// the by-name branch — types that implement
    /// `std::error::Error` are still flagged via the trait branch.
    ignore_suffixes: Vec<String>,
}

pub struct ExhaustiveErrorEnums {
    require_for: RequireFor,
    suffixes: BTreeSet<String>,
}

impl ExhaustiveErrorEnums {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let suffixes = resolve_string_set(
            DEFAULT_SUFFIXES,
            config.extra_suffixes,
            config.ignore_suffixes,
        );
        Self {
            require_for: config.require_for,
            suffixes,
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

impl_lint_pass!(ExhaustiveErrorEnums => [EXHAUSTIVE_ERROR_ENUMS]);

impl RuleRegistration for ExhaustiveErrorEnumsRule {
    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[EXHAUSTIVE_ERROR_ENUMS]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        if let DefaultState::Inactive = resolved_state("exhaustive_error_enums", DEFAULT_STATE) {
            return;
        }
        lint_store.register_late_pass(|_| Box::new(ExhaustiveErrorEnums::new()));
    }
}

impl<'tcx> LateLintPass<'tcx> for ExhaustiveErrorEnums {
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
    let ty = cx
        .tcx
        .type_of(def_id)
        .instantiate_identity()
        .skip_normalization();
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
    let field_ty = cx
        .tcx
        .type_of(fields[0].def_id)
        .instantiate_identity()
        .skip_normalization();
    matches!(field_ty.kind(), ty::Adt(adt_def, _) if adt_def.is_enum())
}

fn emit(cx: &LateContext<'_>, item: &hir::Item<'_>, kind_label: &str, ident: rustc_span::Ident) {
    let name = ident.name.as_str();
    let message = format!("{kind_label} `{name}` is missing `#[non_exhaustive]`");
    let insert_at = item.span.shrink_to_lo();
    let indent = indent_of(cx, item.span).unwrap_or(0);
    let replacement = format!("#[non_exhaustive]\n{:indent$}", "", indent = indent);
    span_lint_and_then(cx, EXHAUSTIVE_ERROR_ENUMS, ident.span, message, |diag| {
        diag.span_suggestion(
            insert_at,
            "add `#[non_exhaustive]` to keep new variants from being a SemVer break",
            replacement,
            Applicability::MaybeIncorrect,
        );
    });
}
