use crate::common::DefaultState;
use crate::rule_index::{Register, rule};
use crate::test_code::item_in_test_code;
use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Item, ItemKind, VariantData};
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

declare_tool_lint! {
    /// ### What it does
    ///
    /// Counts the fields of every struct — named or tuple — and flags a
    /// struct with more than `max_fields` (default `10`).
    ///
    /// A struct produced by a macro expansion is not measured. Enum
    /// variants and unions are not measured.
    ///
    /// Test code is measured like any other code; set
    /// `test_code_exception` to leave it alone.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. A
    /// struct with many fields is a function with that many parameters
    /// in disguise: every constructor names them all, every reader of
    /// the type holds them all, and a change to one is a change to a
    /// type that everything depends on. Past a handful the fields fall
    /// into groups — the paths, the credentials, the retry policy — and
    /// each group is a smaller struct with a name of its own, which the
    /// functions that only need that group can then take instead.
    ///
    /// A settings struct that mirrors a configuration file is the usual
    /// exception; allow it at the site with a `reason` that says so.
    ///
    /// ### Interaction with Clippy
    ///
    /// `clippy::struct_excessive_bools` (`pedantic`) caps the `bool`
    /// fields alone, as a signal that a struct is really a state
    /// machine; `clippy::too_many_arguments` caps a function's
    /// parameters. Neither caps a struct's fields as a whole.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// struct Client {
    ///     base_url: Url,
    ///     token: Option<String>,
    ///     username: Option<String>,
    ///     password: Option<String>,
    ///     ca_file: Option<PathBuf>,
    ///     cert_file: Option<PathBuf>,
    ///     key_file: Option<PathBuf>,
    ///     retries: u32,
    ///     retry_factor: f64,
    ///     retry_min_timeout: Duration,
    ///     retry_max_timeout: Duration,
    ///     timeout: Duration,
    /// }
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// struct Client {
    ///     base_url: Url,
    ///     auth: Auth,
    ///     tls: Tls,
    ///     retry: RetryPolicy,
    ///     timeout: Duration,
    /// }
    /// ```
    pub perfectionist::TOO_MANY_STRUCT_FIELDS,
    Warn,
    "struct has more fields than the configured maximum",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::too_many_struct_fields";

/// A struct whose constructor still fits in one look.
const DEFAULT_MAX_FIELDS: usize = 10;

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// The most fields a struct may have without being flagged.
    /// Defaults to `10`.
    max_fields: usize,
    /// Whether test code is left alone: structs inside a `#[cfg(test)]`
    /// module or an integration-test or benchmark target. Defaults to
    /// `false`, so a test fixture is held to the same limit as the code
    /// it exercises.
    test_code_exception: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_fields: DEFAULT_MAX_FIELDS,
            test_code_exception: false,
        }
    }
}

pub struct TooManyStructFields {
    config: Config,
}

impl_lint_pass!(TooManyStructFields => [TOO_MANY_STRUCT_FIELDS]);

impl Register for rule::TooManyStructFields {
    const DEFAULT_STATE: DefaultState = DefaultState::Active;

    fn register_lint(lint_store: &mut LintStore) {
        lint_store.register_lints(&[TOO_MANY_STRUCT_FIELDS]);
    }

    fn register_pass(lint_store: &mut LintStore) {
        lint_store.register_late_lint_pass(Box::new(|_| {
            Box::new(TooManyStructFields {
                config: dylint_linting::config_or_default(CONFIG_KEY),
            })
        }));
    }
}

impl<'tcx> LateLintPass<'tcx> for TooManyStructFields {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let ItemKind::Struct(ident, _, data) = item.kind else {
            return;
        };
        let count = match data {
            VariantData::Struct { fields, .. } | VariantData::Tuple(fields, ..) => fields.len(),
            VariantData::Unit(..) => return,
        };
        if count <= self.config.max_fields || item.span.from_expansion() {
            return;
        }
        if self.config.test_code_exception && item_in_test_code(cx, item.owner_id.def_id) {
            return;
        }
        let max = self.config.max_fields;
        let name = ident.name;
        let noun = if count == 1 { "field" } else { "fields" };
        let message = format!("struct `{name}` has {count} {noun}, above the limit of {max}");
        span_lint_and_help(
            cx,
            TOO_MANY_STRUCT_FIELDS,
            cx.tcx.def_span(item.owner_id.def_id),
            message,
            None,
            "group related fields into a struct of their own, or split the type by the concerns its fields serve",
        );
    }
}
