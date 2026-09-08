//! Run compiletest UI fixtures from a throwaway copy so the committed
//! `.rs` fixtures stay pristine while their `.stderr` files keep
//! normalised `line:column` numbers.
//!
//! `dylint_testing` drives rustc under `-Zui-testing`, which anonymises
//! the source-line gutter to `LL` but leaves the real `line:column` in
//! every `--> .../<file>.rs:LINE:COL` span header. Those numbers churn
//! whenever a line is inserted above a diagnostic. compiletest can
//! rewrite them, but only through a per-file `// normalize-stderr-test`
//! header directive — a regex applied to the driver's actual output
//! before it is diffed against the committed `.stderr`. Rather than
//! commit that directive into every fixture, [`copy_fixtures_with_directive`]
//! injects it into a temporary copy at test time.
//!
//! # Keeping the copy's path legible
//!
//! compiletest names each test after the *last component* of its
//! `src_base`, followed by the fixture's path relative to that
//! `src_base`. Handing it the temp dir itself therefore names every
//! test after a random string:
//!
//! ```text
//! test [ui] .tmpRisvpQ/fixture.rs ... ok
//! ```
//!
//! which says nothing about which fixture ran. So the copy reproduces
//! the fixture's repository-relative path *inside* the temp dir, and
//! [`FixtureCopy::path`] points at the copy of the path's first
//! component rather than at the temp dir. compiletest then walks down
//! to the fixture through the remaining components and spells the whole
//! relative path in the test name:
//!
//! ```text
//! test [ui] ui-toml/allow_attributes/apply_to_outer_scopes/fixture.rs ... ok
//! ```

use crate::TempDir;
use std::fs;
use std::path::{Path, PathBuf};

/// The compiletest header directive that collapses each
/// `.rs:LINE:COL` in a fixture's actual output to `.rs:LL:CC`. It is a
/// plain `//` line comment (not a doc comment) carrying no URL, e-mail,
/// `#issue`, backtick, or repo ref, and no Unicode ellipsis, so none of
/// perfectionist's text-scanning lints fires on it.
const NORMALIZE_STDERR_DIRECTIVE: &str =
    r#"// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC""#;

/// A throwaway copy of a fixture tree, laid out so that compiletest
/// names each test after the fixture's repository-relative path.
///
/// Hold it until the test has run: dropping it deletes the copy.
pub struct FixtureCopy {
    /// Held only for its `Drop`, which removes the copy from disk.
    _temp: TempDir,
    /// The directory to hand to `dylint_testing::ui::Test::src_base`.
    src_base: PathBuf,
}

impl FixtureCopy {
    /// The `src_base` to hand to `dylint_testing::ui::Test::src_base`.
    /// It is the copy of the *first* component of the path passed to
    /// [`copy_fixtures_with_directive`], not the temp dir and not the
    /// fixture directory itself, so that the components in between end
    /// up in compiletest's test names.
    pub fn path(&self) -> &Path {
        &self.src_base
    }
}

/// Copy the fixture directory `<manifest_dir>/<relative>` into a fresh
/// [`TempDir`] — reproducing `relative` inside it — and prepend
/// [`NORMALIZE_STDERR_DIRECTIVE`] to every `.rs` that has a sibling
/// `.stderr`. Pass the returned guard's [`FixtureCopy::path`] to
/// `dylint_testing::ui::Test::src_base`, and hold the guard until the
/// test has run so the copy outlives the assertions.
///
/// Only `.rs` files paired with a `.stderr` are touched, so `auxiliary/`
/// crates and `include!`-ed sources are copied verbatim — the injected
/// directive on the paired fixture already rewrites every header in that
/// fixture's output, wherever the span originates.
pub fn copy_fixtures_with_directive(manifest_dir: &str, relative: &str) -> FixtureCopy {
    let relative = Path::new(relative);
    let temp = TempDir::new().expect("create fixture copy dir");
    let destination = temp.path().join(relative);
    copy_dir(&Path::new(manifest_dir).join(relative), &destination);
    inject_directive(&destination);
    // compiletest recurses from `src_base`, and the copy holds nothing
    // but `relative`, so starting at the first component still collects
    // exactly the fixtures that were copied.
    let root = relative
        .components()
        .next()
        .expect("fixture path is not empty");
    let src_base = temp.path().join(root);
    FixtureCopy {
        _temp: temp,
        src_base,
    }
}

/// Recursively copy the contents of `source` into `destination`,
/// preserving the directory layout so `aux-build:` and `include!`
/// references still resolve in the copy.
fn copy_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create fixture copy subdir");
    for entry in fs::read_dir(source).expect("read fixture dir") {
        let entry = entry.expect("read fixture dir entry");
        let from = entry.path();
        let into = destination.join(entry.file_name());
        if entry.file_type().expect("fixture entry file type").is_dir() {
            copy_dir(&from, &into);
        } else {
            fs::copy(&from, &into).expect("copy fixture file");
        }
    }
}

/// Prepend [`NORMALIZE_STDERR_DIRECTIVE`] to every `<name>.rs` under
/// `dir` that has a sibling `<name>.stderr`.
fn inject_directive(dir: &Path) {
    for entry in fs::read_dir(dir).expect("read fixture copy dir") {
        let entry = entry.expect("read fixture copy entry");
        let path = entry.path();
        if entry.file_type().expect("fixture copy file type").is_dir() {
            inject_directive(&path);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && path.with_extension("stderr").exists()
        {
            let body = fs::read_to_string(&path).expect("read fixture .rs");
            fs::write(&path, format!("{NORMALIZE_STDERR_DIRECTIVE}\n{body}"))
                .expect("write fixture .rs");
        }
    }
}
