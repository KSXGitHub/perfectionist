use super::{cargo_command, pinned_requirement};
use semver::Version;
use std::collections::BTreeSet;
use std::path::Path;

/// The variables the spawned command has to clear, spelled out here
/// rather than read from `UI_HARNESS_VARS`. Asserting against the very
/// list the command is built from would pass just as happily if that
/// list were emptied, which is the regression this test exists to catch.
const EXPECTED: &[&str] = &["DYLINT_LIBRARY_PATH", "DYLINT_LIBS", "DYLINT_TOML"];

/// Each of [`EXPECTED`] must reach the spawned command as a removal
/// rather than as an inherited value. `Command::get_envs` reports every
/// explicit change the builder made, pairing a key with `None` when the
/// command clears it, so the `None`-valued keys are what the subprocess
/// will not see.
#[test]
fn every_ui_harness_var_is_cleared() {
    let command = cargo_command(Path::new("/project"), Path::new("/target"));
    let cleared: BTreeSet<&str> = command
        .get_envs()
        .filter(|(_, value)| value.is_none())
        .filter_map(|(key, _)| key.to_str())
        .collect();
    assert_eq!(cleared, EXPECTED.iter().copied().collect());
}

/// A pin with no operator admits the one release, which is how
/// `cargo install --version` reads it: `install-dev-tools` hands the
/// pin to that flag, so the check has to agree with what the flag
/// installs rather than with the `6.x` range the string would mean to
/// a dependency resolver.
#[test]
fn a_bare_pin_admits_that_release_alone() {
    let pin = pinned_requirement("6.1.0").expect("parse a bare pin");
    assert!(pin.matches(&Version::new(6, 1, 0)));
    assert!(!pin.matches(&Version::new(6, 1, 1)));
    assert!(!pin.matches(&Version::new(6, 2, 0)));
}

/// A pin carrying an operator keeps that operator's range, so
/// relaxing the manifest to a caret requirement relaxes the check
/// with it.
#[test]
fn an_operator_pin_keeps_its_range() {
    let pin = pinned_requirement("^6.1").expect("parse a caret pin");
    assert!(pin.matches(&Version::new(6, 1, 1)));
    assert!(pin.matches(&Version::new(6, 2, 0)));
    assert!(!pin.matches(&Version::new(7, 0, 0)));
}

/// A pin that is neither a version nor a requirement has no range to
/// check against, which the caller reports rather than treating as a
/// match.
#[test]
fn an_unparsable_pin_yields_no_requirement() {
    assert!(pinned_requirement("not-a-version").is_none());
}
