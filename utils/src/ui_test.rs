//! Run a compiletest UI fixture under a `dylint.toml` the test
//! supplies, holding the process-global state that carries it for
//! exactly one run. [`ConfiguredUiTest::builder`] is the way in.
//!
//! `dylint_testing` configures its driver through `DYLINT_TOML`, a
//! process-global environment variable it sets for the duration of
//! one fixture run and restores afterwards. Two runs must not overlap
//! on it: the second fixture would be linted under the first's
//! configuration while still being diffed against its own `.stderr`,
//! and so judged on a baseline it never asked for.
//!
//! The version of `dylint_testing` this crate pins already prevents
//! that by itself, taking a private static mutex as the first
//! statement of the function that sets the variable and holding it
//! until the driver has run. Nothing in the crate's API promises it,
//! though, so a version bump could drop it with nothing here
//! noticing.
//!
//! [`ConfiguredUiTestBuilder::run`] therefore takes a lock of this
//! crate's own and holds it across the fixture run it wraps.
//! `dylint_testing` is a dependency of this crate and not of the lint
//! crate whose fixtures it runs, so a test binary cannot name the
//! builder to reach around that lock: the guarantee is this
//! repository's own rather than borrowed, and it holds by compilation
//! rather than by every new test file being told about it.
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

/// A `dylint_testing` UI test, described and ready to run.
pub struct ConfiguredUiTest {
    test: dylint_testing::ui::Test,
    /// Held only for its `Drop`, which removes the fixture copy from
    /// disk.
    _fixtures: FixtureCopy,
    /// Held only for its `Drop`, which releases [`SERIAL`].
    _serial: MutexGuard<'static, ()>,
}

impl ConfiguredUiTest {
    /// Start describing a UI test.
    pub fn builder() -> ConfiguredUiTestBuilder<(), (), (), ()> {
        ConfiguredUiTestBuilder {
            library_name: (),
            manifest_dir: (),
            src_base: (),
            dylint_toml: (),
            rustc_flags: Vec::new(),
        }
    }

    pub(crate) fn run(mut self) {
        self.test.run();
    }
}

/// The parameters of a [`ConfiguredUiTest`], collected before any of
/// them is used: nothing here touches the fixture tree or the lock
/// until [`ConfiguredUiTestBuilder::run`] is called.
///
/// Each type parameter is the slot of the setter that fills it, `()`
/// until then, so a builder missing a setter has no `run` to call and
/// the omission is a compile error rather than a panic on a
/// half-described test.
#[must_use = "a `ConfiguredUiTestBuilder` describes a test until it is `run`"]
pub struct ConfiguredUiTestBuilder<LibraryName, ManifestDir, SrcBase, DylintToml> {
    library_name: LibraryName,
    manifest_dir: ManifestDir,
    src_base: SrcBase,
    dylint_toml: DylintToml,
    rustc_flags: Vec<String>,
}

impl<ManifestDir, SrcBase, DylintToml>
    ConfiguredUiTestBuilder<(), ManifestDir, SrcBase, DylintToml>
{
    /// The dylint library to load: the calling test binary's
    /// `CARGO_PKG_NAME`.
    pub fn library_name<LibraryName: AsRef<str>>(
        self,
        library_name: LibraryName,
    ) -> ConfiguredUiTestBuilder<LibraryName, ManifestDir, SrcBase, DylintToml> {
        let Self {
            manifest_dir,
            src_base,
            dylint_toml,
            rustc_flags,
            ..
        } = self;
        ConfiguredUiTestBuilder {
            library_name,
            manifest_dir,
            src_base,
            dylint_toml,
            rustc_flags,
        }
    }
}

impl<LibraryName, SrcBase, DylintToml>
    ConfiguredUiTestBuilder<LibraryName, (), SrcBase, DylintToml>
{
    /// The directory the fixture tree is relative to: the calling test
    /// binary's `CARGO_MANIFEST_DIR`. It has to be absolute.
    pub fn manifest_dir<ManifestDir: AsRef<str>>(
        self,
        manifest_dir: ManifestDir,
    ) -> ConfiguredUiTestBuilder<LibraryName, ManifestDir, SrcBase, DylintToml> {
        let Self {
            library_name,
            src_base,
            dylint_toml,
            rustc_flags,
            ..
        } = self;
        ConfiguredUiTestBuilder {
            library_name,
            manifest_dir,
            src_base,
            dylint_toml,
            rustc_flags,
        }
    }
}

impl<LibraryName, ManifestDir, DylintToml>
    ConfiguredUiTestBuilder<LibraryName, ManifestDir, (), DylintToml>
{
    /// The fixture tree to lint, relative to the manifest directory.
    pub fn src_base<SrcBase: AsRef<str>>(
        self,
        src_base: SrcBase,
    ) -> ConfiguredUiTestBuilder<LibraryName, ManifestDir, SrcBase, DylintToml> {
        let Self {
            library_name,
            manifest_dir,
            dylint_toml,
            rustc_flags,
            ..
        } = self;
        ConfiguredUiTestBuilder {
            library_name,
            manifest_dir,
            src_base,
            dylint_toml,
            rustc_flags,
        }
    }
}

impl<LibraryName, ManifestDir, SrcBase>
    ConfiguredUiTestBuilder<LibraryName, ManifestDir, SrcBase, ()>
{
    /// The `dylint.toml` the fixtures are linted under. Pass an empty
    /// string to pin them to the default configuration rather than let
    /// the harness fall back to the linted crate's own file.
    pub fn dylint_toml<DylintToml: AsRef<str>>(
        self,
        dylint_toml: DylintToml,
    ) -> ConfiguredUiTestBuilder<LibraryName, ManifestDir, SrcBase, DylintToml> {
        let Self {
            library_name,
            manifest_dir,
            src_base,
            rustc_flags,
            ..
        } = self;
        ConfiguredUiTestBuilder {
            library_name,
            manifest_dir,
            src_base,
            dylint_toml,
            rustc_flags,
        }
    }
}

impl<LibraryName, ManifestDir, SrcBase, DylintToml>
    ConfiguredUiTestBuilder<LibraryName, ManifestDir, SrcBase, DylintToml>
{
    /// Pass flags to the compiler that lints the fixtures. Optional,
    /// so it sits outside the typestate; repeated calls accumulate.
    pub fn rustc_flags(mut self, rustc_flags: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.rustc_flags
            .extend(rustc_flags.into_iter().map(|flag| flag.as_ref().to_owned()));
        self
    }
}

impl<LibraryName, ManifestDir, SrcBase, DylintToml>
    ConfiguredUiTestBuilder<LibraryName, ManifestDir, SrcBase, DylintToml>
where
    LibraryName: AsRef<str>,
    ManifestDir: AsRef<str>,
    SrcBase: AsRef<str>,
    DylintToml: AsRef<str>,
{
    /// Build the test and run it.
    ///
    /// This is the only way out of the builder: a built
    /// [`ConfiguredUiTest`] holds the lock and offers nothing to do
    /// but run, so handing one out could only widen the window it is
    /// held for.
    pub fn run(self) {
        self.build().run();
    }

    /// Everything the builder deferred happens here.
    fn build(self) -> ConfiguredUiTest {
        // The copy lands in a `TempDir` of its own and shares nothing,
        // so it stays outside the critical section.
        let fixtures =
            copy_fixtures_with_directives(self.manifest_dir.as_ref(), self.src_base.as_ref());
        let serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let mut test =
            dylint_testing::ui::Test::src_base(self.library_name.as_ref(), fixtures.path());
        test.dylint_toml(self.dylint_toml);
        test.rustc_flags(self.rustc_flags);
        ConfiguredUiTest {
            test,
            _fixtures: fixtures,
            _serial: serial,
        }
    }
}
