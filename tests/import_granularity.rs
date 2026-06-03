//! UI tests for `import_granularity`'s configuration knobs. The
//! default-config (`style = "module"`) sweep lives in
//! `ui/import_granularity.rs` and is picked up by `tests/ui.rs`; these
//! tests each point at their own one-fixture directory under
//! `ui-toml/import_granularity/` and pass a per-rule `dylint.toml` to
//! [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared [`Mutex`]
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::import_granularity";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept separate from the lint's own internal `Config`
/// so the test surface is independent of the implementation's private
/// struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    respect_cfg_blocks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    respect_visibility: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    respect_doc_comments: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    self_merge: Option<&'static str>,
}

fn dylint_toml(config: RuleConfig) -> String {
    // The rule is active by default, so unlike the opt-in rules' test
    // harnesses no `[perfectionist] enable = [...]` table is needed —
    // only the per-rule `[perfectionist::import_granularity]` knobs.
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, config: RuleConfig) {
    // A poisoned mutex from a previous panic doesn't make this lock
    // unsafe — recover the inner guard and proceed.
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn crate_style_collapses_per_root() {
    run(
        "ui-toml/import_granularity/crate_style",
        RuleConfig {
            style: Some("crate"),
            ..Default::default()
        },
    );
}

#[test]
fn item_style_splits_per_leaf() {
    run(
        "ui-toml/import_granularity/item_style",
        RuleConfig {
            style: Some("item"),
            ..Default::default()
        },
    );
}

#[test]
fn self_merge_fold_enforces_self() {
    // `crate` style with `self_merge = "fold"`: a name that is both an
    // item and a module is always written `crate::thing::{self, T}`. The
    // sibling-split single statement is flagged and rewritten to it.
    run(
        "ui-toml/import_granularity/self_merge_fold",
        RuleConfig {
            style: Some("crate"),
            self_merge: Some("fold"),
            ..Default::default()
        },
    );
}

#[test]
fn self_merge_split_enforces_siblings() {
    // `crate` style with `self_merge = "split"`: the same name is always
    // written `crate::{thing, thing::T}`. The `self`-fold single
    // statement is flagged and rewritten to it.
    run(
        "ui-toml/import_granularity/self_merge_split",
        RuleConfig {
            style: Some("crate"),
            self_merge: Some("split"),
            ..Default::default()
        },
    );
}

#[test]
fn respect_doc_comments_false_allows_merge() {
    // Default `module` style, but the doc-commented `use` is now
    // allowed to merge with its plain same-module neighbour.
    run(
        "ui-toml/import_granularity/respect_doc_comments",
        RuleConfig {
            respect_doc_comments: Some(false),
            ..Default::default()
        },
    );
}

#[test]
fn respect_visibility_false_flags_without_fixing() {
    // With visibility ignored for grouping, a `pub use` and a private
    // `use` from the same module are flagged together — but the fix is
    // withheld because merging can't preserve both visibilities.
    run(
        "ui-toml/import_granularity/respect_visibility",
        RuleConfig {
            respect_visibility: Some(false),
            ..Default::default()
        },
    );
}

#[test]
fn respect_cfg_blocks_false_flags_without_fixing() {
    // With cfg gates ignored for grouping, a platform-gated `use` and an
    // unconditional one from the same module are flagged together — but
    // the fix is withheld because merging would drop the `#[cfg]` gate.
    run(
        "ui-toml/import_granularity/respect_cfg_blocks",
        RuleConfig {
            respect_cfg_blocks: Some(false),
            ..Default::default()
        },
    );
}
