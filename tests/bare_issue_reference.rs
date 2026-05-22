//! UI tests for `bare_issue_reference`'s configuration knobs. See
//! the module docs on `tests/bare_url.rs` for the shared pattern.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::bare_issue_reference";

static SERIAL: Mutex<()> = Mutex::new(());

#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    repo_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    form: Option<String>,
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
fn github_base_url_emits_machine_applicable_inline_link() {
    // With `repo_base_url = "https://github.com/owner/repo"` and the
    // default `form = "inline"`, the rule emits a
    // `MachineApplicable` suggestion of the form
    // `[#NNN](https://github.com/owner/repo/issues/NNN)`. GitHub
    // redirects `/issues/<n>` to `/pull/<n>` when the number names
    // a PR, which is why the inline-form / GitHub-host combination
    // is the only path that earns `MachineApplicable`.
    run(
        "ui-toml/bare_issue_reference/github_inline",
        RuleConfig {
            repo_base_url: Some("https://github.com/owner/repo".into()),
            ..Default::default()
        },
    );
}

#[test]
fn reference_form_always_degrades_to_maybe_incorrect() {
    // The `form = "reference"` suggestion produces just `[#NNN]`,
    // without the matching `[#NNN]: URL` definition (which the
    // author must add). Applying that suggestion as-is would
    // leave the doc block with an undefined reference link — so
    // applicability degrades to `MaybeIncorrect` even on GitHub,
    // where the inline form would otherwise be machine-applicable.
    run(
        "ui-toml/bare_issue_reference/reference_form",
        RuleConfig {
            repo_base_url: Some("https://github.com/owner/repo".into()),
            form: Some("reference".into()),
        },
    );
}
