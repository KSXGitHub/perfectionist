//! UI tests for `wildcard_imports`'s configuration knobs. The
//! default-config sweep (both exceptions enabled) lives in
//! `ui/wildcard_imports.rs` and is picked up by `tests/ui.rs`; these tests
//! each point at their own one-fixture directory under
//! `ui-toml/wildcard_imports/` and pass a per-rule `dylint.toml` to
//! [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for the
//! duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared [`Mutex`]
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::wildcard_imports";

static SERIAL: Mutex<()> = Mutex::new(());

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    prelude_exception: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_reexport_exception: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prelude_segment_names: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_paths: Option<Vec<&'static str>>,
}

fn dylint_toml(config: RuleConfig) -> String {
    // The rule is active by default, so no `[perfectionist] enable`
    // table is needed — only the per-rule knobs.
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, config: RuleConfig) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    _utils::scratch::redirect_temp_dir();
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn exceptions_disabled_flags_every_glob() {
    // Both exceptions off: the prelude glob and the `pub` root re-export
    // glob lose their exemption and are flagged alongside the plain glob.
    run(
        "ui-toml/wildcard_imports/exceptions_disabled",
        RuleConfig {
            prelude_exception: Some(false),
            root_reexport_exception: Some(false),
            ..Default::default()
        },
    );
}

#[test]
fn allowed_paths_exempts_listed_module() {
    // With both exceptions off, a glob is normally flagged — except one
    // whose absolute module path is named in `allowed_paths`. A crate-root
    // entry is written `crate::...` (no leading `::`), while an extern entry
    // carries a leading `::`, and `::std::collections` exempts both the
    // plain and the `::`-rooted spelling of that glob.
    run(
        "ui-toml/wildcard_imports/allowed_paths",
        RuleConfig {
            prelude_exception: Some(false),
            root_reexport_exception: Some(false),
            allowed_paths: Some(vec!["crate::secret::internals", "::std::collections"]),
            ..Default::default()
        },
    );
}

#[test]
fn custom_prelude_segment_name() {
    // `prelude_segment_names = ["api"]`: `use foo::api::*` becomes exempt
    // while `use foo::prelude::*` loses its exemption and is flagged.
    run(
        "ui-toml/wildcard_imports/custom_prelude_segment",
        RuleConfig {
            prelude_segment_names: Some(vec!["api"]),
            ..Default::default()
        },
    );
}
