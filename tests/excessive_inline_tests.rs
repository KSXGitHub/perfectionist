//! Integration tests for `excessive_inline_tests`.
//!
//! Each test materialises a minimal Cargo project under a `TempDir` and
//! runs `cargo dylint --all -- --all-targets` against it (sharing the
//! warmed `target/integration-fixtures`). `--all-targets` is mandatory:
//! the rule can only observe `#[cfg(test)]` / `#[test]` code in a build
//! where `cfg(test)` is active, which is the unit-test target that flag
//! adds. Per-rule configuration is appended to the fixture's
//! `dylint.toml` as a quoted `["perfectionist::excessive_inline_tests"]`
//! table; pass `""` for the default configuration.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};

const LINT: &str = "perfectionist::excessive_inline_tests";

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
        "fixture_itf_inline_over_budget",
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
        "fixture_itf_inline_under_budget",
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

/// An external `#[cfg(test)] mod tests;` is already extracted, so it is
/// neutral for the footprint: it charges nothing and the all-test file
/// it points at is itself exempt. No warning regardless of where on
/// disk the extracted file sits.
#[test]
fn does_not_flag_external_test_module() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_itf_external_module",
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

/// A `mod tests;` gated by a *compound* cfg that still implies test
/// (`#[cfg(all(test, unix))]`) is an external test-module declaration
/// just like a bare `#[cfg(test)]`, so it must be recognised as test
/// and stay neutral — never re-flagged as inline test code living in a
/// production file. This is the false positive of
/// <https://github.com/KSXGitHub/perfectionist/issues/187>: a `use`
/// in the test file would otherwise count as production and the
/// `#[test] fn` as misplaced inline test code.
#[test]
fn does_not_flag_external_module_gated_by_compound_cfg() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_itf_external_compound_cfg",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub mod foo;\n"),
            (
                "src/foo.rs",
                "pub fn parse() -> i32 {\n    1\n}\n\n#[cfg(all(test, unix))]\nmod tests;\n",
            ),
            (
                "src/foo/tests.rs",
                "use super::parse;\n\n#[test]\nfn works() { assert_eq!(parse(), 1); }\n",
            ),
        ],
        "[\"perfectionist::excessive_inline_tests\"]\ninline_style = \"external_only\"\n",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "a compound-cfg-gated external test module must not be flagged; stderr was:\n{stderr}",
    );
}

/// A compound cfg that does *not* imply test — `#[cfg(any(test,
/// unix))]` — gates real production code: the item is compiled in a
/// non-test `unix` build too, so it is not test-only. The rule must
/// descend into such a module and count its body as production, never
/// charge it against the inline-test budget. With the `any`/`all`
/// composition bug it was misread as test-only, its (large) body left
/// uncounted, and the whole module reported as "inline test code spans
/// N lines". Regression for
/// <https://github.com/KSXGitHub/perfectionist/issues/211>.
#[test]
fn does_not_flag_production_module_gated_by_any_test_cfg() {
    let mut foo = String::from("#[cfg(any(test, unix))]\nmod gated {\n");
    for index in 0..60 {
        foo.push_str(&format!("    pub fn item_{index}() -> i32 {{ {index} }}\n"));
    }
    foo.push_str("}\n");
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_itf_any_test_cfg_production",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[("src/lib.rs", "pub mod foo;\n"), ("src/foo.rs", &foo)],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "a production module gated by `cfg(any(test, ...))` must not be \
         flagged as inline test code; stderr was:\n{stderr}",
    );
}

/// The on-disk position of an extracted test file is not the rule's
/// concern: a flattened `src/sib_tests.rs` sibling (loaded via `#[path]`)
/// is just as acceptable as the nested form. The rule only counts
/// inline footprint, so this draws no warning.
#[test]
fn does_not_flag_external_module_regardless_of_path() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_itf_external_sibling_path",
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
        !stderr.contains(LINT),
        "the on-disk path of an extracted test file is not checked; stderr was:\n{stderr}",
    );
}

#[test]
fn exempts_file_of_only_test_items() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_itf_all_test_file",
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
/// harness injects synthetic crate-root items (the generated [`main`],
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
        "fixture_itf_all_test_crate_root",
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
        "fixture_itf_macro_production",
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
        "fixture_itf_named_module_help",
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

/// A small integration-test body with a top-level `use` and helper
/// `fn` (both counted as production, which defeats the all-test-file
/// exemption) followed by 60 `#[test] fn`s — well over the inline
/// budget. Reused by the flat and subdirectory layout tests below.
fn integration_test_body() -> String {
    let mut body = String::from("use std::cmp::max;\n\nfn helper() -> i32 {\n    max(1, 0)\n}\n\n");
    for index in 0..60 {
        body.push_str(&format!(
            "#[test]\nfn case_{index}() {{ assert_eq!(helper(), 1); }}\n",
        ));
    }
    body
}

/// Integration tests live in their own crate under `tests/`, which a
/// `--all-targets` run hands the rule too. Their top-level `#[test]`
/// functions are the target itself, not unit tests misplaced inside a
/// production file, so the rule must not flag them — even when the
/// test footprint dwarfs the inline budget and the file also carries
/// top-level `use`s and helper `fn`s the rule would otherwise count as
/// production.
#[test]
fn does_not_flag_integration_tests() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_itf_integration_tests",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub fn calculate() -> i32 {\n    1\n}\n"),
            ("tests/integration.rs", &integration_test_body()),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "integration tests under `tests/` must not be flagged; stderr was:\n{stderr}",
    );
}

/// Cargo also auto-discovers the subdirectory form `tests/<name>/main.rs`
/// as an integration-test crate. Its discriminating `tests` directory is
/// one level further up than the flat form, so this guards the path
/// arithmetic that walks past the `main.rs` stem.
#[test]
fn does_not_flag_integration_test_subdir_main() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_itf_integration_subdir",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub fn calculate() -> i32 {\n    1\n}\n"),
            ("tests/suite/main.rs", &integration_test_body()),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "the `tests/<name>/main.rs` form must not be flagged; stderr was:\n{stderr}",
    );
}

/// A flat integration test may be named [`main`] (`tests/main.rs`),
/// which is *not* the `tests/<name>/main.rs` subdirectory form: its
/// discriminating `tests` directory is the immediate parent, not two
/// levels up. It must still be recognised as a separate target.
#[test]
fn does_not_flag_integration_test_named_main() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_itf_integration_named_main",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub fn calculate() -> i32 {\n    1\n}\n"),
            ("tests/main.rs", &integration_test_body()),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "an integration test named `main` must not be flagged; stderr was:\n{stderr}",
    );
}

/// Positive control for the integration-test cases above: the same body
/// in a `src/`-rooted module *is* over budget and flagged. Without it,
/// those negatives could pass vacuously — a missing warning is otherwise
/// indistinguishable from the rule never running on the crate.
#[test]
fn flags_equivalent_body_in_src_module() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_itf_src_body_positive_control",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub mod thing;\n"),
            ("src/thing.rs", &integration_test_body()),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("inline test code spans") && stderr.contains("src/thing/tests.rs"),
        "the same over-budget body in a `src/` module must be flagged with the inline-footprint \
         diagnostic; stderr was:\n{stderr}",
    );
}

/// Benchmarks are their own crate compiled under `cfg(test)`, like
/// integration tests, so without the separate-target skip the same
/// over-budget body would be flagged. This is the one load-bearing
/// separate-target branch besides `tests/`, so guard it directly.
#[test]
fn does_not_flag_benchmarks() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_itf_benchmarks",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub fn calculate() -> i32 {\n    1\n}\n"),
            ("benches/bench.rs", &integration_test_body()),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "benchmarks under `benches/` must not be flagged; stderr was:\n{stderr}",
    );
}

#[test]
fn external_only_flags_inline_tests() {
    let (_temp, stderr, success) = run_project_with_config(
        "fixture_itf_external_only",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("src/lib.rs", "pub mod small;\n"),
            (
                "src/small.rs",
                "pub fn negate(value: i32) -> i32 {\n    -value\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn works() { assert_eq!(super::negate(1), -1); }\n}\n",
            ),
        ],
        "[\"perfectionist::excessive_inline_tests\"]\ninline_style = \"external_only\"\n",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains("inline test code should live in an external module"),
        "expected the external_only message; stderr was:\n{stderr}",
    );
}
