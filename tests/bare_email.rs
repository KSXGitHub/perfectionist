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
    skip_domains: Option<Vec<String>>,
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
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
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
            skip_domains: Some(vec!["EXAMPLE.COM".to_owned()]),
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
