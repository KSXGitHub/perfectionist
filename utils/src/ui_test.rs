//! Run a compiletest UI fixture under a `dylint.toml` the test
//! supplies, holding the process-global state that carries it for
//! exactly one run.
//!
//! `dylint_testing` configures its driver through `DYLINT_TOML`, an
//! environment variable it sets for the duration of one fixture run
//! and restores afterwards. The variable is process-global and the
//! default test harness runs a binary's `#[test]`s in parallel
//! threads, so two runs that overlapped would lint one fixture under
//! the other's configuration. Nothing would report that: the fixture
//! is diffed against its own `.stderr` either way, so it passes or
//! fails on a baseline it never asked for.
//!
//! [`configured_ui_test`] takes [`SERIAL`] and hands the guard to the
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
//! `DYLINT_LIBRARY_PATH` and `DYLINT_LIBS` are the other half of the
//! problem, and no lock can scope them; a spawned `cargo dylint`
//! clears them instead. See [`crate::dylint`].

use crate::ui_fixtures::{FixtureCopy, copy_fixtures_with_directives};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Serialises the window in which `DYLINT_TOML` names one fixture's
/// configuration.
///
/// A poisoned lock is recovered rather than unwrapped: the guard
/// protects an environment variable the panicking run has already
/// restored, so a later fixture inherits nothing corrupt from it.
/// Recovering does not keep the binary's remaining `#[test]`s green —
/// `compiletest` panics inside `dylint_testing`'s own mutex, poisoning
/// that one, and the next run unwraps it — but the failures that
/// follow are then upstream's to explain rather than this lock's.
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

    /// Run the fixtures, then delete the copy and release the lock.
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
/// The returned value holds [`SERIAL`] until it runs, so build it
/// where the test runs: a second one alive on the same thread would
/// block on the guard the first is holding.
pub fn configured_ui_test(
    library_name: &str,
    manifest_dir: &str,
    src_base: &str,
    dylint_toml: impl AsRef<str>,
) -> ConfiguredUiTest {
    // The copy lands in a `TempDir` of its own and shares nothing, so
    // it stays outside the critical section.
    let fixtures = copy_fixtures_with_directives(manifest_dir, src_base);
    let serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    let mut test = dylint_testing::ui::Test::src_base(library_name, fixtures.path());
    test.dylint_toml(dylint_toml);
    ConfiguredUiTest {
        test,
        _fixtures: fixtures,
        _serial: serial,
    }
}
