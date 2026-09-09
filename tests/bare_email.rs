//! UI tests for `bare_email`'s configuration knobs. See the module
//! docs on `tests/bare_url.rs` for the shared pattern.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::bare_email";

static SERIAL: Mutex<()> = Mutex::new(());

#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_domains: Option<Vec<&'static str>>,
}

fn dylint_toml(config: RuleConfig) -> String {
    #[derive(serde::Serialize)]
    struct WholeToml<'a> {
        #[serde(flatten)]
        rule: BTreeMap<&'a str, RuleConfig>,
    }
    let whole = WholeToml {
        rule: [(LINT_NAME, config)].into_iter().collect(),
    };
    toml::to_string(&whole).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, config: RuleConfig) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    let fixtures = _utils::copy_fixtures_with_directive(env!("CARGO_MANIFEST_DIR"), src_base);
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn skip_domains_matches_case_insensitively() {
    // `skip_domains` compares case-insensitively (per DNS): an
    // uppercase `EXAMPLE.COM` entry suppresses a lowercase
    // `user@example.com`, while an address on another domain fires.
    run(
        "ui-toml/bare_email/skip_domains_case_insensitive",
        RuleConfig {
            skip_domains: Some(vec!["EXAMPLE.COM"]),
            ..Default::default()
        },
    );
}

#[test]
fn mailto_style_emits_single_mailto_suggestion() {
    // `style = "mailto"` produces one `MachineApplicable` suggestion
    // that prefixes the address with `mailto:`. Distinguished from
    // the default `either` style, which emits two `MaybeIncorrect`
    // suggestions.
    run(
        "ui-toml/bare_email/mailto_style",
        RuleConfig {
            style: Some("mailto".into()),
            ..Default::default()
        },
    );
}

#[test]
fn both_style_emits_single_combined_suggestion() {
    // `style = "both"` produces one `MachineApplicable` suggestion
    // that applies the angle-bracket and `mailto:` forms together
    // (`<mailto:...>`), rather than offering them as alternatives the
    // way the default `either` style does.
    run(
        "ui-toml/bare_email/both_style",
        RuleConfig {
            style: Some("both".into()),
            ..Default::default()
        },
    );
}

#[test]
fn forbid_style_emits_no_suggestion() {
    // `style = "forbid"` is the one style whose contract is *no*
    // autofix: no address form is acceptable, so the diagnostic
    // carries prose only. The fixture guards that contract — a future
    // edit that attaches a suggestion to this arm changes the
    // expected output.
    run(
        "ui-toml/bare_email/forbid_style",
        RuleConfig {
            style: Some("forbid".into()),
            ..Default::default()
        },
    );
}

/// Regression test for
/// <https://github.com/KSXGitHub/perfectionist/issues/165>: a per-item
/// `#[expect]` on the documented item both suppresses the bare-email
/// finding in its doc comment and is fulfilled by it. The fixture
/// produces no diagnostics; before the fix the finding resolved to the
/// crate root, firing anyway and leaving the expectation unfulfilled.
#[test]
fn per_item_expect_fulfils_and_suppresses() {
    run("ui-toml/bare_email/expect_at_item", RuleConfig::default());
}
