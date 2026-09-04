use super::collect_rules;
use crate::model::DefaultState;
use std::fs;
use std::path::PathBuf;

/// Allocate a fresh temp directory unique across both processes
/// (cargo's test harness forks per binary) and across tests in
/// the same binary (the atomic counter handles concurrent runs
/// and any label collision). Mirrors the helper in
/// `check_md.rs`'s test module; kept local so the two test
/// modules stay self-contained.
fn tempdir(label: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!(
        "perfectionist-gen-docs-extract-{label}-{}-{seq}",
        std::process::id(),
    ));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();
    base
}

/// Directory-module rules keep `CONFIG_KEY` and `Config` in
/// `<rule>/config.rs`. Merging the submodule items into the
/// parent file is what lets [`extract_config`] find them; this
/// pins the behaviour so the bug that sent
/// `unicode_ellipsis_in_panic_messages` to "Configuration: none."
/// can't silently come back.
#[test]
fn collect_rules_finds_config_in_submodule_file() {
    // Nested so `rules_dir.parent()` is an empty stand-in `src/`,
    // not the shared temp root that `collect_rules` would scan.
    let base = tempdir("merge-submodule");
    let rules_dir = base.join("rules");
    fs::create_dir_all(&rules_dir).unwrap();
    let parent_path = rules_dir.join("demo_rule.rs");
    fs::write(
        &parent_path,
        r#"
            use rustc_session::declare_tool_lint;

            mod config;

            declare_tool_lint! {
                /// ### What it does
                /// Demo.
                pub perfectionist::DEMO_RULE,
                Warn,
                "demo description"
            }
        "#,
    )
    .unwrap();
    let submodule_dir = rules_dir.join("demo_rule");
    fs::create_dir_all(&submodule_dir).unwrap();
    fs::write(
        submodule_dir.join("config.rs"),
        r#"
            const CONFIG_KEY: &str = "perfectionist::demo_rule";

            #[derive(serde::Deserialize)]
            #[serde(default, rename_all = "snake_case")]
            struct Config {
                /// Demo knob.
                knob: bool,
            }
        "#,
    )
    .unwrap();

    let rules = collect_rules(&rules_dir);
    assert_eq!(rules.len(), 1, "exactly one rule should be discovered");
    let config = &rules[0].config;
    assert_eq!(config.key, "perfectionist::demo_rule");
    let names: Vec<&str> = config.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, vec!["knob"]);

    let _ = fs::remove_dir_all(&base);
}

/// The default-state axis lives in the rule's `Register` impl, so
/// reading it means walking an associated const rather than a
/// module-level one. Miss that and every off-by-default rule renders
/// as shipping on.
#[test]
fn collect_rules_reads_default_state_from_the_register_impl() {
    let base = tempdir("impl-default-state");
    let rules_dir = base.join("rules");
    fs::create_dir_all(&rules_dir).unwrap();
    fs::write(
        rules_dir.join("demo_rule.rs"),
        r#"
            use rustc_session::declare_tool_lint;

            declare_tool_lint! {
                /// ### What it does
                /// Demo.
                pub perfectionist::DEMO_RULE,
                Warn,
                "demo description"
            }

            const CONFIG_KEY: &str = "perfectionist::demo_rule";

            #[derive(serde::Deserialize)]
            #[serde(default, rename_all = "snake_case")]
            struct Config {
                /// Demo knob.
                knob: bool,
            }

            impl Register for rule::DemoRule {
                const DEFAULT_STATE: DefaultState = DefaultState::Inactive;

                fn register_lint(lint_store: &mut LintStore) {}

                fn register_pass(lint_store: &mut LintStore) {}
            }
        "#,
    )
    .unwrap();

    let rules = collect_rules(&rules_dir);
    assert_eq!(rules.len(), 1, "exactly one rule should be discovered");
    assert!(
        matches!(rules[0].default_state, DefaultState::Inactive),
        "default state should come from the impl, got {:?}",
        rules[0].default_state,
    );

    let _ = fs::remove_dir_all(&base);
}
