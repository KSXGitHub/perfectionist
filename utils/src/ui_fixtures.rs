//! Run compiletest UI fixtures from a throwaway copy so the committed
//! `.rs` fixtures stay pristine while their `.stderr` files stay free
//! of the churn and the stray whitespace the driver's raw output
//! carries.
//!
//! `dylint_testing` drives rustc under `-Zui-testing`, which anonymises
//! the source-line gutter to `LL` but leaves everything else verbatim:
//! the real `line:column` in every `--> .../<file>.rs:LINE:COL` span
//! header, the real count in the closing warning tally, a trailing
//! space on each rendered suggestion line that inserts a blank line,
//! and a blank line at the very end. compiletest can rewrite all of
//! that, but only through per-file `// normalize-stderr-test` header
//! directives — each a regex applied to the driver's actual output
//! before it is diffed against the committed `.stderr`. Rather than
//! commit those directives into every fixture,
//! [`copy_fixtures_with_directives`] injects them into a temporary
//! copy at test time.

use crate::TempDir;
use std::fs;
use std::path::{Path, PathBuf};

/// The compiletest header directives that rewrite a fixture's actual
/// output before it is diffed, applied in the order given. Each is a
/// plain `//` line comment (not a doc comment) carrying no URL, e-mail,
/// `#issue`, backtick, or repo ref, and no Unicode ellipsis, so none of
/// perfectionist's text-scanning lints fires on it.
///
/// What each one buys, in order:
///
/// 1. `.rs:LINE:COL` becomes `.rs:LL:CC`, so inserting a line above a
///    diagnostic no longer renumbers every span header below it.
/// 2. The closing tally becomes `warning: N warnings emitted`, so
///    adding or removing a case no longer rewrites a line that counts
///    the diagnostics already spelled out above it. The singular
///    `1 warning emitted` collapses into the same text, which is the
///    point: the line is meant to read the same in every fixture.
/// 3. Trailing spaces come off every line. A suggestion that inserts a
///    blank line renders as an `LL + ` row with nothing after the `+`,
///    which `.editorconfig` has every conforming editor strip on save,
///    breaking a committed fixture that depended on it.
/// 4. A run of newlines at the end collapses to one, so each `.stderr`
///    ends like every other text file in the tree. Its capture group is
///    what keeps the surviving newline: a compiletest replacement is
///    substituted literally apart from `$`-references, so spelling it
///    `\n` would insert a backslash and an `n`.
///
/// Rules 3 and 4 are ordered: taking the spaces off first turns a
/// whitespace-only last line into an empty one for rule 4 to absorb.
const NORMALIZE_STDERR_DIRECTIVES: &[&str] = &[
    r#"// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC""#,
    r#"// normalize-stderr-test: "(?m)^warning: \d+ warnings? emitted$" -> "warning: N warnings emitted""#,
    r#"// normalize-stderr-test: "(?m) +$" -> """#,
    r#"// normalize-stderr-test: "(\n)\n+\z" -> "$1""#,
];

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
    /// [`copy_fixtures_with_directives`], not the temp dir and not the
    /// fixture directory itself, so that the components in between end
    /// up in compiletest's test names.
    pub fn path(&self) -> &Path {
        &self.src_base
    }
}

/// Copy the fixture directory `<manifest_dir>/<relative>` into a fresh
/// [`TempDir`] — reproducing `relative` inside it — and prepend
/// [`NORMALIZE_STDERR_DIRECTIVES`] to every `.rs` that has a sibling
/// `.stderr`. Pass the returned guard's [`FixtureCopy::path`] to
/// `dylint_testing::ui::Test::src_base`, and hold the guard until the
/// test has run so the copy outlives the assertions.
///
/// Only `.rs` files paired with a `.stderr` are touched, so `auxiliary/`
/// crates and `include!`-ed sources are copied verbatim — the injected
/// directives on the paired fixture already rewrite that fixture's whole
/// output, wherever a span originates.
pub fn copy_fixtures_with_directives(manifest_dir: &str, relative: &str) -> FixtureCopy {
    let relative = Path::new(relative);
    let temp = TempDir::new().expect("create fixture copy dir");
    let destination = temp.path().join(relative);
    copy_dir(&Path::new(manifest_dir).join(relative), &destination);
    inject_directives(&destination);
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

/// Prepend [`NORMALIZE_STDERR_DIRECTIVES`] to every `<name>.rs` under
/// `dir` that has a sibling `<name>.stderr`.
fn inject_directives(dir: &Path) {
    for entry in fs::read_dir(dir).expect("read fixture copy dir") {
        let entry = entry.expect("read fixture copy entry");
        let path = entry.path();
        if entry.file_type().expect("fixture copy file type").is_dir() {
            inject_directives(&path);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && path.with_extension("stderr").exists()
        {
            let body = fs::read_to_string(&path).expect("read fixture .rs");
            let header = NORMALIZE_STDERR_DIRECTIVES.join("\n");
            fs::write(&path, format!("{header}\n{body}")).expect("write fixture .rs");
        }
    }
}
