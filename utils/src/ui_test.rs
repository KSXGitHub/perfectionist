//! Run a compiletest UI fixture under a `dylint.toml` the test
//! supplies, holding the process-global state that carries it for
//! exactly one run.
//!
//! `dylint_testing` configures its driver through `DYLINT_TOML`, an
//! environment variable it sets for the duration of one fixture run
//! and restores afterwards. The variable is process-global and the
//! default test harness runs a binary's `#[test]`s in parallel
//! threads, so two runs that overlapped would lint one fixture under
//! the other's configuration and diff it against the wrong `.stderr`
//! — silently, since neither run panics or hangs.
//!
//! [`configured_ui_test`] takes [`SERIAL`] before it builds the
//! `dylint_testing` builder and hands the guard to the
//! [`ConfiguredUiTest`] it returns, which holds it until
//! [`ConfiguredUiTest::run`] consumes the value. `dylint_testing` is a
//! dependency of this crate and not of the lint crate whose fixtures
//! it runs, so a test binary cannot name the builder to reach around
//! the lock — the overlap is a compile error rather than a convention
//! every new test file has to be told about.
//!
//! The version of `dylint_testing` this crate pins serialises its own
//! fixture runs on a private static of the same shape, so the overlap
//! is already unreachable there. That static is an implementation
//! detail rather than part of the crate's API, which is what the lock
//! here carries across a version bump.
//!
//! `DYLINT_LIBRARY_PATH` and `DYLINT_LIBS` are the same problem with a
//! different answer: `dylint_testing` sets them once, on the first UI
//! test, and never restores them, so no lock can scope them and a
//! spawned `cargo dylint` has to clear them instead. That side lives
//! in [`crate::dylint`].

use crate::ui_fixtures::{FixtureCopy, copy_fixtures_with_directives};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Serialises the window in which `DYLINT_TOML` names one fixture's
/// configuration.
///
/// A poisoned lock is taken anyway: the guard protects an environment
/// variable that the panicking run has already restored, so there is
/// no corrupted state for a later fixture to inherit, and failing
/// every remaining `#[test]` in the binary would bury the one that
/// actually failed.
static SERIAL: Mutex<()> = Mutex::new(());

/// A `dylint_testing` UI test that has taken [`SERIAL`], together with
/// the throwaway fixture copy it runs against. Both are released when
/// [`ConfiguredUiTest::run`] consumes the value.
#[must_use = "a `ConfiguredUiTest` lints nothing until it is `run`"]
pub struct ConfiguredUiTest {
    test: dylint_testing::ui::Test,
    /// Held only for its `Drop`, which removes the copy from disk.
    _fixtures: FixtureCopy,
    /// Held only for its `Drop`, which releases [`SERIAL`].
    _serial: MutexGuard<'static, ()>,
}

impl ConfiguredUiTest {
    /// Pass flags to the compiler that lints the fixtures.
    pub fn rustc_flags(mut self, rustc_flags: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.test.rustc_flags(rustc_flags);
        self
    }

    /// Run the fixtures, then release the lock and delete the copy.
    pub fn run(mut self) {
        self.test.run();
    }
}

/// Prepare the fixture tree at `<manifest_dir>/<src_base>` — copied
/// and rewritten by [`copy_fixtures_with_directives`] — as a UI test
/// of the dylint library `library_name`, configured by `dylint_toml`.
///
/// `library_name` and `manifest_dir` are the calling test binary's
/// `CARGO_PKG_NAME` and `CARGO_MANIFEST_DIR`; this crate is built in
/// isolation from the workspace under test and cannot read either.
///
/// The returned value holds [`SERIAL`], so build it where the test
/// runs rather than storing it.
pub fn configured_ui_test(
    library_name: &str,
    manifest_dir: &str,
    src_base: &str,
    dylint_toml: impl AsRef<str>,
) -> ConfiguredUiTest {
    let serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    let fixtures = copy_fixtures_with_directives(manifest_dir, src_base);
    let mut test = dylint_testing::ui::Test::src_base(library_name, fixtures.path());
    test.dylint_toml(dylint_toml);
    ConfiguredUiTest {
        test,
        _fixtures: fixtures,
        _serial: serial,
    }
}

#[cfg(test)]
mod tests;
