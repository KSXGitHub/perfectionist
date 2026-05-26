//! Integration tests for `unit_test_file_layout`.
//!
//! Each test materialises a minimal Cargo project under a `TempDir` and
//! runs `cargo dylint --all -- --all-targets` against it (sharing the
//! warmed `target/integration-fixtures`). `--all-targets` is mandatory:
//! the rule can only observe `#[cfg(test)]` / `#[test]` code in a build
//! where `cfg(test)` is active, which is the unit-test target that flag
//! adds. Per-rule configuration is appended to the fixture's
//! `dylint.toml` as a quoted `["perfectionist::unit_test_file_layout"]`
//! table; pass `""` for the default configuration.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};

const LINT: &str = "perfectionist::unit_test_file_layout";

/// Build a `src/big.rs` whose inline `mod tests` block alone is well
/// over the default 50-line budget.
fn big_inline_test_file() -> String {
    let mut source =
        String::from("pub fn calculate() -> i32 {\n    1\n}\n\n#[cfg(test)]\nmod tests {\n");
    for index in 0..60 {
        source.push_str(&format!(
            "    #[test]\n    fn case_{index}() {{ assert_eq!(super::calculate(), 1); }}\n",
        ));
    }
    source.push_str("}\n");
    source
}

#[test]
fn flags_inline_footprint_over_budget() {
    let big = big_inline_test_file();
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_inline_over_budget",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[("src/lib.rs", "pub mod big;\n"), ("src/big.rs", &big)],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains(LINT),
        "expected `{LINT}` warning; stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains("inline test code spans"),
        "expected the over-budget message; stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains("src/big/tests.rs"),
        "expected the canonical extraction target in the help; stderr was:\n{stderr}",
    );
}

#[test]
fn does_not_flag_small_inline_footprint() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_inline_under_budget",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub mod small;\n"),
            (
                "src/small.rs",
                "pub fn negate(value: i32) -> i32 {\n    -value\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn works() { assert_eq!(super::negate(1), -1); }\n}\n",
            ),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "did not expect a `{LINT}` warning; stderr was:\n{stderr}",
    );
}

#[test]
fn does_not_flag_external_nested_layout() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_external_nested",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub mod foo;\n"),
            (
                "src/foo.rs",
                "pub fn parse() -> i32 {\n    1\n}\n\n#[cfg(test)]\nmod tests;\n",
            ),
            (
                "src/foo/tests.rs",
                "#[test]\nfn works() { assert_eq!(super::parse(), 1); }\n",
            ),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "did not expect a `{LINT}` warning; stderr was:\n{stderr}",
    );
}

#[test]
fn flags_external_module_in_sibling_location() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_external_sibling_under_nested",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub mod sib;\n"),
            (
                "src/sib.rs",
                "pub fn parse() -> i32 {\n    1\n}\n\n#[cfg(test)]\n#[path = \"sib_tests.rs\"]\nmod tests;\n",
            ),
            (
                "src/sib_tests.rs",
                "#[test]\nfn works() { assert_eq!(super::parse(), 1); }\n",
            ),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("not in the canonical nested location"),
        "expected the layout message; stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains("src/sib/tests.rs"),
        "expected the nested target in the help; stderr was:\n{stderr}",
    );
}

#[test]
fn flags_unexpected_sibling() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_unexpected_sibling",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub mod good;\n"),
            (
                "src/good.rs",
                "pub fn parse() -> i32 {\n    1\n}\n\n#[cfg(test)]\nmod tests;\n",
            ),
            (
                "src/good/tests.rs",
                "#[test]\nfn works() { assert_eq!(super::parse(), 1); }\n",
            ),
            (
                "src/good_tests.rs",
                "// stray file from a half-done migration\n",
            ),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("unexpected sibling test file"),
        "expected the unexpected-sibling message; stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains("src/good_tests.rs"),
        "expected the stray sibling path in the help; stderr was:\n{stderr}",
    );
}

/// A `<stem>_<name>.rs` that is itself a live module (loaded by its own
/// `mod` declaration) is not a migration straggler, so it must not be
/// flagged for deletion even when a correct nested test file coexists.
#[test]
fn does_not_flag_loaded_module_as_unexpected_sibling() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_loaded_not_sibling",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub mod good;\npub mod good_tests;\n"),
            (
                "src/good.rs",
                "pub fn parse() -> i32 {\n    1\n}\n\n#[cfg(test)]\nmod tests;\n",
            ),
            (
                "src/good/tests.rs",
                "#[test]\nfn works() { assert_eq!(super::parse(), 1); }\n",
            ),
            (
                "src/good_tests.rs",
                "pub fn unrelated_helper() -> i32 {\n    2\n}\n",
            ),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains("unexpected sibling test file"),
        "a genuinely loaded module must not be flagged as a stray sibling; stderr was:\n{stderr}",
    );
}

#[test]
fn exempts_file_of_only_test_items() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_all_test_file",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub mod fixtures;\n"),
            (
                "src/fixtures.rs",
                "#[cfg(test)]\nmod inner {\n    #[test]\n    fn works() {}\n}\n\n#[cfg(test)]\nfn helper() {}\n",
            ),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "a file of only test items must be exempt; stderr was:\n{stderr}",
    );
}

/// The crate root itself, when it contains only inline test code, must
/// stay exempt. The rule runs in the `cfg(test)` build, where the test
/// harness injects synthetic crate-root items (the generated `main`,
/// `extern crate test`, the descriptor const); those must not be
/// counted as production and rob the file of its exemption.
#[test]
fn exempts_all_test_crate_root() {
    let mut lib = String::from("#[cfg(test)]\nmod tests {\n");
    for index in 0..60 {
        lib.push_str(&format!(
            "    #[test]\n    fn case_{index}() {{ assert!(true); }}\n",
        ));
    }
    lib.push_str("}\n");
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_all_test_crate_root",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[("src/lib.rs", &lib)],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "a crate root of only test items must be exempt despite harness-injected items; \
         stderr was:\n{stderr}",
    );
}

/// A file whose only production item is produced by a *user* macro
/// expansion must still fail the all-test exemption — the expanded
/// production item is counted (charged to the macro's call-site file),
/// so an over-budget inline test block there is flagged.
#[test]
fn flags_inline_tests_when_production_is_macro_generated() {
    let mut foo = String::from("crate::make_thing!();\n\n#[cfg(test)]\nmod tests {\n");
    for index in 0..60 {
        foo.push_str(&format!(
            "    #[test]\n    fn case_{index}() {{ assert!(true); }}\n",
        ));
    }
    foo.push_str("}\n");
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_macro_production",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            (
                "src/lib.rs",
                "#[macro_export]\nmacro_rules! make_thing {\n    () => {\n        pub fn thing() -> i32 {\n            0\n        }\n    };\n}\n\npub mod foo;\n",
            ),
            ("src/foo.rs", &foo),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("inline test code spans"),
        "macro-generated production must not grant the all-test exemption; stderr was:\n{stderr}",
    );
}

/// The extraction help names the inline module's own file, not a
/// hard-coded `tests`: an over-budget `#[cfg(test)] mod edge_cases`
/// should point at `src/foo/edge_cases.rs`.
#[test]
fn help_names_the_actual_inline_module() {
    let mut foo =
        String::from("pub fn calculate() -> i32 {\n    1\n}\n\n#[cfg(test)]\nmod edge_cases {\n");
    for index in 0..60 {
        foo.push_str(&format!(
            "    #[test]\n    fn case_{index}() {{ assert_eq!(super::calculate(), 1); }}\n",
        ));
    }
    foo.push_str("}\n");
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_named_module_help",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[("src/lib.rs", "pub mod foo;\n"), ("src/foo.rs", &foo)],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("src/foo/edge_cases.rs") && stderr.contains("mod edge_cases;"),
        "help should name the actual module (`edge_cases`), not `tests`; stderr was:\n{stderr}",
    );
}

#[test]
fn external_only_flags_inline_tests() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_external_only",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub mod small;\n"),
            (
                "src/small.rs",
                "pub fn negate(value: i32) -> i32 {\n    -value\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn works() { assert_eq!(super::negate(1), -1); }\n}\n",
            ),
        ],
        "[\"perfectionist::unit_test_file_layout\"]\ninline_style = \"external_only\"\n",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("inline test code should live in an external module"),
        "expected the external_only message; stderr was:\n{stderr}",
    );
}

#[test]
fn sibling_layout_accepts_flattened_form() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_sibling_layout",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub mod sib;\n"),
            (
                "src/sib.rs",
                "pub fn parse() -> i32 {\n    1\n}\n\n#[cfg(test)]\n#[path = \"sib_tests.rs\"]\nmod tests;\n",
            ),
            (
                "src/sib_tests.rs",
                "#[test]\nfn works() { assert_eq!(super::parse(), 1); }\n",
            ),
        ],
        "[\"perfectionist::unit_test_file_layout\"]\nexternal_layout = \"sibling\"\n",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "sibling layout must accept the flattened form; stderr was:\n{stderr}",
    );
}

#[test]
fn any_layout_skips_the_location_check() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_utfl_any_layout",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub mod sib;\n"),
            (
                "src/sib.rs",
                "pub fn parse() -> i32 {\n    1\n}\n\n#[cfg(test)]\n#[path = \"sib_tests.rs\"]\nmod tests;\n",
            ),
            (
                "src/sib_tests.rs",
                "#[test]\nfn works() { assert_eq!(super::parse(), 1); }\n",
            ),
        ],
        "[\"perfectionist::unit_test_file_layout\"]\nexternal_layout = \"any\"\n",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "the `any` layout must skip the location check; stderr was:\n{stderr}",
    );
}
