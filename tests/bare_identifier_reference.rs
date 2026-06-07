//! Configuration UI test for `bare_identifier_reference`. The default-config
//! sweep lives in `ui/bare_identifier_reference.rs` and is picked up by
//! `tests/ui.rs`; the test here points at a fixture directory under
//! `ui-toml/bare_identifier_reference/` and passes a per-rule `dylint.toml`.
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared [`Mutex`]
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::bare_identifier_reference";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept as a separate type from the lint's own internal
/// [`Config`](../src/rules/bare_identifier_reference.rs) so the test surface is
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
        "ui-toml/bare_identifier_reference/skip_idents",
        &dylint_toml(RuleConfig {
            skip_idents: Some(vec!["Skipped".to_owned()]),
        }),
    );
}

/// The import-policy fixtures deliberately import across module
/// boundaries, which trips the sibling import-layout rules; disable
/// those so the fixture stays focused on `bare_identifier_reference`.
fn reference_scope_toml(reference_scope: &str) -> String {
    format!(
        "[perfectionist]\n\
         disable = [\"import_grouping\", \"import_granularity\"]\n\
         \n\
         [\"perfectionist::bare_identifier_reference\"]\n\
         reference_scope = \"{reference_scope}\"\n",
    )
}

/// `reference_scope = "own_module"` flags only names defined directly in
/// the documenting item's module; every `use`-imported name is left
/// alone.
#[test]
fn reference_scope_own_module_checks_only_local_definitions() {
    run(
        "ui-toml/bare_identifier_reference/imports_ignore",
        &reference_scope_toml("own_module"),
    );
}

/// `reference_scope = "module_tree"` additionally flags imports from the
/// module's own subtree, but still leaves names reaching outside it
/// (`crate::`, `super::`, other crates) alone.
#[test]
fn reference_scope_module_tree_checks_the_modules_own_subtree() {
    run(
        "ui-toml/bare_identifier_reference/imports_internal",
        &reference_scope_toml("module_tree"),
    );
}

/// `reference_scope = "anywhere"` flags every resolving name over the
/// same source, including the `crate::`-reaching and another-crate
/// imports the narrower policies drop.
#[test]
fn reference_scope_anywhere_checks_everything() {
    run(
        "ui-toml/bare_identifier_reference/imports_anywhere",
        &reference_scope_toml("anywhere"),
    );
}

/// `reference_scope = "crate"` (the default) flags every first-party
/// target (including the `crate::`-reaching one), but drops the
/// another-crate ([`std`]) import.
#[test]
fn reference_scope_crate_checks_the_whole_current_crate() {
    run(
        "ui-toml/bare_identifier_reference/imports_crate",
        &reference_scope_toml("crate"),
    );
}

/// `reference_scope = "third_party"` drops only the standard library;
/// over this fixture (whose only external reference is `std`) that
/// leaves the same first-party set as `crate`. The `crate` vs
/// `third_party` difference on a real dependency is pinned by
/// [`reference_scope_crate_drops_a_dependency`] /
/// [`reference_scope_third_party_flags_a_dependency`].
#[test]
fn reference_scope_third_party_excludes_only_the_standard_library() {
    run(
        "ui-toml/bare_identifier_reference/imports_third_party",
        &reference_scope_toml("third_party"),
    );
}

/// `reference_scope = "crate"` drops a reference to a genuine
/// third-party dependency (loaded via `aux-build`), keeping only the
/// first-party `LocalThing`.
#[test]
fn reference_scope_crate_drops_a_dependency() {
    run(
        "ui-toml/bare_identifier_reference/dependency_crate",
        &reference_scope_toml("crate"),
    );
}

/// `reference_scope = "third_party"` flags the same dependency
/// reference (`Reach::ThirdPartyCrate`) alongside the first-party one —
/// the only behavioural difference from `crate`.
#[test]
fn reference_scope_third_party_flags_a_dependency() {
    run(
        "ui-toml/bare_identifier_reference/dependency_third_party",
        &reference_scope_toml("third_party"),
    );
}

/// A public macro colliding with a private function of the same name is
/// ambiguous, and the disambiguator must name an eligible namespace
/// (`macro@`), not the private `value@` or the absent `type@`.
#[test]
fn ambiguity_disambiguator_picks_an_eligible_namespace() {
    run(
        "ui-toml/bare_identifier_reference/macro_disambiguator",
        &dylint_toml(RuleConfig::default()),
    );
}

/// With all three case knobs off, the conformist names are left alone
/// but a non-conformist name still fires.
#[test]
fn case_knobs_off_keep_only_non_conformist() {
    run(
        "ui-toml/bare_identifier_reference/case_filters",
        "[perfectionist]\n\
         \n\
         [\"perfectionist::bare_identifier_reference\"]\n\
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
        "ui-toml/bare_identifier_reference/min_words",
        "[perfectionist]\n\
         \n\
         [\"perfectionist::bare_identifier_reference\"]\n\
         min_words = 3\n",
    );
}
