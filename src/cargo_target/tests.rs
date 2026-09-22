//! Unit tests for the crate-root path arithmetic and the
//! build-script crate-name check, exercised without a compiler
//! context through the pure [`classify`](super::classify).

use super::{CargoTarget, classify};
use std::path::Path;

/// Classify `root` as the crate root of a package's own library,
/// i.e. with a crate name that is not a build script's.
fn library_crate(root: &str) -> CargoTarget {
    classify("some_package", Some(Path::new(root)))
}

#[test]
fn separate_targets_are_recognised() {
    // Flat `<dir>/<name>.rs`, the `<dir>/main.rs` named-`main` case,
    // the `<dir>/<name>/main.rs` subdirectory form, and an absolute
    // integration-test root — for tests, benches, and examples alike.
    for (path, expected) in [
        ("tests/integration.rs", CargoTarget::IntegrationTest),
        ("tests/main.rs", CargoTarget::IntegrationTest),
        ("tests/suite/main.rs", CargoTarget::IntegrationTest),
        ("/abs/tests/integration.rs", CargoTarget::IntegrationTest),
        ("benches/bench.rs", CargoTarget::Benchmark),
        ("benches/suite/main.rs", CargoTarget::Benchmark),
        ("examples/demo.rs", CargoTarget::Example),
        ("examples/demo/main.rs", CargoTarget::Example),
    ] {
        assert_eq!(
            library_crate(path),
            expected,
            "`{path}` should classify as `{expected:?}`",
        );
    }
}

#[test]
fn library_and_binary_roots_are_recognised() {
    // Library, default binary, extra binaries (flat and the
    // `src/bin/<name>/main.rs` multi-file form), and a nested module
    // file all classify as the package's own code.
    for path in [
        "src/lib.rs",
        "src/main.rs",
        "src/bin/cli.rs",
        "src/bin/cli/main.rs",
        "src/rules/excessive_inline_tests/paths.rs",
        "lib.rs",
    ] {
        assert_eq!(
            library_crate(path),
            CargoTarget::LibOrBin,
            "`{path}` should classify as a library or binary",
        );
    }
}

#[test]
fn build_scripts_are_recognised_by_crate_name() {
    // The default `build.rs` and a `build = "mk.rs"` rename, plus
    // the absence of a crate root — a build script is recognised by
    // Cargo's crate-name prefix, not by where its root sits.
    for (crate_name, root) in [
        ("build_script_build", Some("build.rs")),
        ("build_script_mk", Some("mk.rs")),
        ("build_script_build", None),
    ] {
        assert_eq!(
            classify(crate_name, root.map(Path::new)),
            CargoTarget::BuildScript,
            "`{crate_name}` should classify as a build script",
        );
    }
}

#[test]
fn a_package_root_is_never_read_as_a_target_directory() {
    // Anything under `src/` is the package's own code, however its
    // directories are named. rustc is handed a member's crate root
    // relative to the workspace root, so a member directory named like
    // a target directory would otherwise swallow the whole crate.
    for path in [
        "examples/src/main.rs",
        "tests/src/main.rs",
        "benches/src/main.rs",
        "tests/mycrate/src/main.rs",
        // A binary named after a target directory, in both of the
        // shapes Cargo auto-discovers under `src/bin`.
        "src/bin/tests/main.rs",
        "src/bin/benches/main.rs",
        "src/bin/examples.rs",
        // An ancestor named `src` must not move the anchor off the
        // package's own `src`.
        "/home/user/src/proj/src/bin/tests/main.rs",
    ] {
        assert_eq!(
            library_crate(path),
            CargoTarget::LibOrBin,
            "`{path}` should classify as a library or binary",
        );
    }
}

#[test]
fn a_target_declared_under_src_is_still_a_separate_target() {
    // Nothing stops a `[[test]] path = ...` entry from pointing under
    // `src/`; only Cargo's own `lib.rs` / `main.rs` / `bin` layouts
    // there belong to the package itself.
    for (path, expected) in [
        ("src/tests/it.rs", CargoTarget::IntegrationTest),
        ("src/benches/bench.rs", CargoTarget::Benchmark),
        ("src/examples/demo.rs", CargoTarget::Example),
        (
            "/home/user/src/proj/tests/it.rs",
            CargoTarget::IntegrationTest,
        ),
    ] {
        assert_eq!(
            library_crate(path),
            expected,
            "`{path}` should classify as `{expected:?}`",
        );
    }
}

#[test]
fn a_target_named_after_another_target_directory_uses_the_right_one() {
    // Both components name a target directory, so only their order
    // says which is Cargo's and which is the target's own name.
    for (path, expected) in [
        ("examples/tests/main.rs", CargoTarget::Example),
        ("tests/examples/main.rs", CargoTarget::IntegrationTest),
        ("benches/examples/main.rs", CargoTarget::Benchmark),
    ] {
        assert_eq!(
            library_crate(path),
            expected,
            "`{path}` should classify as `{expected:?}`",
        );
    }
}

#[test]
fn a_test_crate_named_like_a_build_script_stays_an_integration_test() {
    // Cargo names an integration test after its file, so this crate
    // name carries the build-script prefix without being one. The
    // path is what tells them apart.
    assert_eq!(
        classify(
            "build_script_env",
            Some(Path::new("tests/build_script_env.rs")),
        ),
        CargoTarget::IntegrationTest,
    );
}

#[test]
fn a_missing_crate_root_is_not_a_separate_target() {
    // `local_crate_source_file` is `None` when the crate came from
    // stdin or a virtual file; treat that as ordinary code rather
    // than exempting it.
    assert_eq!(classify("some_package", None), CargoTarget::LibOrBin);
}

#[test]
fn kind_predicates_agree_with_the_variants() {
    assert!(CargoTarget::IntegrationTest.is_test_target());
    assert!(CargoTarget::Benchmark.is_test_target());
    // An example is documentation, not test code.
    assert!(!CargoTarget::Example.is_test_target());
    assert!(!CargoTarget::BuildScript.is_test_target());
    assert!(!CargoTarget::LibOrBin.is_test_target());

    assert!(CargoTarget::IntegrationTest.is_separate_target());
    assert!(CargoTarget::Benchmark.is_separate_target());
    assert!(CargoTarget::Example.is_separate_target());
    assert!(!CargoTarget::BuildScript.is_separate_target());
    assert!(!CargoTarget::LibOrBin.is_separate_target());
}
