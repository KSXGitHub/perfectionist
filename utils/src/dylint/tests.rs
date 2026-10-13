use super::cargo_command;
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
