//! Configuration UI test for `intra_doc_links`. The default-config
//! sweep lives in `ui/intra_doc_links.rs` and is picked up by
//! `tests/ui.rs`; the test here points at a fixture directory under
//! `ui-toml/intra_doc_links/` and passes a per-rule `dylint.toml`.
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared [`Mutex`]
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::intra_doc_links";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept as a separate type from the lint's own internal
/// [`Config`](../src/rules/intra_doc_links.rs) so the test surface is
/// independent of the implementation's private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_idents: Option<Vec<String>>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, contents: &str) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(contents)
        .run();
}

/// An identifier listed under `skip_idents` is left alone even when it
/// resolves in scope, while a sibling identifier still fires.
#[test]
fn skip_idents_silences_listed_identifiers() {
    run(
        "ui-toml/intra_doc_links/skip_idents",
        &dylint_toml(RuleConfig {
            skip_idents: Some(vec!["Skipped".to_owned()]),
        }),
    );
}

/// The import-policy fixtures deliberately import across module
/// boundaries, which trips the sibling import-layout rules; disable
/// those so the fixture stays focused on `intra_doc_links`.
fn reference_scope_toml(reference_scope: &str) -> String {
    format!(
        "[perfectionist]\n\
         disable = [\"import_grouping\", \"import_granularity\"]\n\
         \n\
         [\"perfectionist::intra_doc_links\"]\n\
         reference_scope = \"{reference_scope}\"\n",
    )
}

/// `reference_scope = "own_module"` flags only names defined directly in
/// the documenting item's module; every `use`-imported name is left
/// alone.
#[test]
fn reference_scope_own_module_checks_only_local_definitions() {
    run(
        "ui-toml/intra_doc_links/imports_ignore",
        &reference_scope_toml("own_module"),
    );
}

/// `reference_scope = "module_tree"` additionally flags imports from the
/// module's own subtree, but still leaves names reaching outside it
/// (`crate::`, `super::`, other crates) alone.
#[test]
fn reference_scope_module_tree_checks_the_modules_own_subtree() {
    run(
        "ui-toml/intra_doc_links/imports_internal",
        &reference_scope_toml("module_tree"),
    );
}

/// With all three case knobs off, the conformist names are left alone
/// but a non-conformist name still fires. (`prefer_expect_over_allow` is
/// disabled because the fixture's non-conformist type needs a naming
/// `#[allow]`.)
#[test]
fn case_knobs_off_keep_only_non_conformist() {
    run(
        "ui-toml/intra_doc_links/case_filters",
        "[perfectionist]\n\
         disable = [\"prefer_expect_over_allow\"]\n\
         \n\
         [\"perfectionist::intra_doc_links\"]\n\
         check_pascal_case = false\n\
         check_upper_case = false\n\
         check_snake_case = false\n",
    );
}

/// `min_words = 3` exempts one- and two-word conformist names, checks
/// three-word ones, and still checks a non-conformist name regardless.
#[test]
fn min_words_threshold_exempts_short_conformist_names() {
    run(
        "ui-toml/intra_doc_links/min_words",
        "[perfectionist]\n\
         disable = [\"prefer_expect_over_allow\"]\n\
         \n\
         [\"perfectionist::intra_doc_links\"]\n\
         min_words = 3\n",
    );
}
