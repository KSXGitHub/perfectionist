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
//! directives. Each directive is a regex applied to the driver's
//! actual output before it is diffed against the committed `.stderr`.
//! Rather than
//! commit those directives into every fixture,
//! [`copy_fixtures_with_directives`] injects them into a temporary
//! copy at test time.
//!
//! The same copy is where a `// run-rustfix` fixture's `.fixed`
//! comes from. compiletest reads the fixed form it compares against
//! from a file beside the fixture, and committing that would commit a
//! second copy of the fixture to show the handful of lines a rewrite
//! touches, so what is committed is a `.diff`, and the `.fixed` the
//! copy holds is written from it.

pub(crate) mod patch;

use crate::TempDir;
use patch::try_apply;
use std::fs;
use std::path::{Component, Path, PathBuf};

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

/// The directive a fixture carries to have rustfix rewrite the copy
/// and compiletest compare that rewrite against a `.fixed`.
const RUN_RUSTFIX: &str = "// run-rustfix";

/// Whether rustfix rewrites `source`, which is the fixture whose
/// fixed form a `.diff` stores.
pub(crate) fn runs_rustfix(source: &str) -> bool {
    source.lines().any(|line| line == RUN_RUSTFIX)
}

/// `text` with the header [`inject_directives`] put in front of it
/// taken back off, which is what a fixture's own text looks like.
///
/// Each directive is required in place rather than counted off, so a
/// header that stops matching fails rather than eating a line of the
/// fixture.
pub(crate) fn strip_directives(text: &str) -> &str {
    let mut rest = text;
    for directive in NORMALIZE_STDERR_DIRECTIVES {
        let (line, tail) = rest
            .split_once('\n')
            .unwrap_or_else(|| panic!("a staged fixture carries the header, got {rest:?}"));
        assert_eq!(
            line, *directive,
            "the header is out of step with what it writes",
        );
        rest = tail;
    }
    rest
}

/// A throwaway copy of a fixture tree, laid out so that compiletest
/// names each test after the fixture's repository-relative path.
///
/// Hold it until the test has run: dropping it deletes the copy.
pub(crate) struct FixtureCopy {
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
    pub(crate) fn path(&self) -> &Path {
        &self.src_base
    }
}

/// Copy the fixture directory `<manifest_dir>/<relative>` into a fresh
/// [`TempDir`], reproducing `relative` inside it, then prepend
/// [`NORMALIZE_STDERR_DIRECTIVES`] to every fixture
/// [`fixture_sources`] collects. The returned guard's
/// `FixtureCopy::path` is the `src_base` the UI harness reads, and the
/// guard has to outlive the run, which is why [`crate::ui_test`] holds
/// it alongside the test.
///
/// An `auxiliary/` crate gets no header of its own: the directives on
/// the fixture that builds against it rewrite that fixture's whole
/// output, wherever a span originates.
pub(crate) fn copy_fixtures_with_directives(manifest_dir: &str, relative: &str) -> FixtureCopy {
    let manifest_dir = Path::new(manifest_dir);
    let relative = Path::new(relative);
    // `Path::join` discards its base when handed an absolute path, and
    // walks out of it when handed `..`. Either way the copy's source and
    // its destination resolve outside the `TempDir`: onto the manifest
    // directory itself, where `copy_dir` would `fs::copy` every file
    // onto itself and truncate it, or onto whatever `..` reaches. So
    // every component of the fixture path has to be an ordinary name,
    // which also rejects the two arguments being passed the wrong way
    // round. Swapping them is how an absolute path is actually
    // reached, `CARGO_MANIFEST_DIR` being absolute.
    assert!(
        manifest_dir.is_absolute(),
        "the manifest dir must be absolute, got {manifest_dir:?}",
    );
    assert!(
        relative
            .components()
            .all(|component| matches!(component, Component::Normal(_))),
        "every component of the fixture path must be an ordinary name, got {relative:?}",
    );
    let temp = TempDir::new().expect("create fixture copy dir");
    let destination = temp.path().join(relative);
    copy_dir(&manifest_dir.join(relative), &destination);
    materialise_fixed(&destination);
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

/// Write the `.fixed` of every `.rs` under `dir` that stores a
/// `.diff`, so compiletest has the fixed form to compare its rustfix
/// output against. The `.diff` is applied to the fixture as
/// committed, before [`inject_directives`] puts a header on either
/// file.
fn materialise_fixed(dir: &Path) {
    for path in fixture_sources(dir) {
        let diff = path.with_extension("diff");
        if !diff.exists() {
            continue;
        }
        let original = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let stored = fs::read_to_string(&diff)
            .unwrap_or_else(|error| panic!("read {}: {error}", diff.display()));
        let what = format!("{}", diff.display());
        let text = match try_apply(&original, &stored, &what) {
            Ok(text) => text,
            // A diff that no longer fits its fixture is the one a
            // blessing run exists to replace, and writing nothing
            // leaves compiletest to stage what rustfix produced.
            Err(_) if *crate::bless::ENABLED => continue,
            Err(reason) => panic!("{reason}"),
        };
        let fixed = path.with_extension("fixed");
        fs::write(&fixed, text)
            .unwrap_or_else(|error| panic!("write {}: {error}", fixed.display()));
    }
}

/// Put `header` and a newline in front of what `file` holds.
fn prepend(file: &Path, header: &str) {
    let body =
        fs::read_to_string(file).unwrap_or_else(|error| panic!("read {}: {error}", file.display()));
    fs::write(file, format!("{header}\n{body}"))
        .unwrap_or_else(|error| panic!("write {}: {error}", file.display()));
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

/// Every fixture under `dir` that compiletest will run. compiletest
/// skips an `auxiliary` directory itself, treating what is in one as
/// something a fixture builds against rather than a test.
///
/// Every caller has to agree on this set: a fixture the header misses
/// is linted at its real line and column numbers, and recording that
/// output would commit them.
pub(crate) fn fixture_sources(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).expect("read fixture dir") {
        let entry = entry.expect("read fixture dir entry");
        let path = entry.path();
        if entry.file_type().expect("fixture file type").is_dir() {
            if entry.file_name() != "auxiliary" {
                found.extend(fixture_sources(&path));
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    found
}

/// Prepend [`NORMALIZE_STDERR_DIRECTIVES`] to every fixture under
/// `dir` that [`fixture_sources`] collects, and to the `.fixed` of
/// one that has it.
fn inject_directives(dir: &Path) {
    let header = NORMALIZE_STDERR_DIRECTIVES.join("\n");
    for path in fixture_sources(dir) {
        prepend(&path, &header);
        // rustfix rewrites this copy, header and all, and compiletest
        // compares that rewrite against the `.fixed`, so a header on
        // one file and not the other fails the comparison on the
        // header itself.
        let fixed = path.with_extension("fixed");
        if fixed.exists() {
            prepend(&fixed, &header);
        }
    }
}

#[cfg(test)]
mod tests;
