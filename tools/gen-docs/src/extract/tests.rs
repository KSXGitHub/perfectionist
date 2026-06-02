use std::path::PathBuf;

use super::*;

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
