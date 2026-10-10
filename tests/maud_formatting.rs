//! Every maud template is written the way `maudfmt` writes it.
//!
//! rustfmt does not enter a `{`-delimited macro invocation, so the
//! layout inside a `maud::html!` body is beyond what `just fmt`
//! settles. A template whose `{` and `}` disagree compiles, renders
//! correctly and passes every other step; it reads wrong, though,
//! because a child indented level with the attributes above it looks
//! like another attribute. That shape had been written twice before
//! anything checked for it.
//!
//! [`maudfmt`](https://docs.rs/maudfmt) settles the whole layout of
//! such a body rather than its braces alone. It is a library as well
//! as a command, so this formats each file in memory and compares,
//! writing nothing and shelling out to nothing. A file holding no
//! maud macro comes back unchanged, which is why every file can be
//! handed over rather than only those naming one.

use command_extra::CommandExtra;
use maudfmt::{FormatOptions, try_fmt_file};
use pipe_trait::Pipe;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Paths, relative to the repository root, that are left unformatted.
///
/// These hold code written to be linted, some of it written to be
/// wrong on purpose, so how it is laid out is the fixture's business.
/// Some of it is not meant to parse either, and `maudfmt` parses a
/// whole file before it reaches any macro in it. Build output and
/// dependencies need no entry: the listing comes from git, so
/// `.gitignore` is the one place that says which directories hold
/// them.
const UNSCANNED: &[&str] = &["ui", "ui-toml", "tests/fixtures"];

/// Run `git` in `root` with `args`, and give back its stdout.
fn git_capture<Args, Arg>(root: &Path, args: Args) -> Vec<u8>
where
    Args: IntoIterator<Item = Arg>,
    Arg: AsRef<OsStr>,
{
    let output = "git"
        .pipe(Command::new)
        .with_current_dir(root)
        .with_args(args)
        .output()
        .expect("failed to invoke `git`");
    assert!(
        output.status.success(),
        "`git` failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    output.stdout
}

/// Every `.rs` file the repository at `root` counts as its own --
/// tracked, or untracked and not ignored -- minus [`UNSCANNED`].
///
/// `--cached` and `--others` each list in order, but one runs after
/// the other, so the listing they make together needs sorting to be
/// read and reported in a stable order.
fn sources(root: &Path) -> Vec<PathBuf> {
    let listing = git_capture(
        root,
        [
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "*.rs",
        ],
    )
    .pipe(String::from_utf8)
    .expect("`git ls-files` produced non-UTF-8 output");
    let mut found: Vec<PathBuf> = listing
        .split('\0')
        .filter(|listed| !listed.is_empty())
        .map(Path::new)
        .filter(|listed| !UNSCANNED.iter().any(|skipped| listed.starts_with(skipped)))
        .map(Path::to_path_buf)
        .collect();
    found.sort();
    found
}

/// The line number, counting from 1, where `written` and `formatted`
/// first part ways.
fn first_difference(written: &str, formatted: &str) -> usize {
    let parting = written
        .lines()
        .zip(formatted.lines())
        .position(|(left, right)| left != right);
    let shortest = written.lines().count().min(formatted.lines().count());
    parting.unwrap_or(shortest) + 1
}

#[test]
fn every_maud_template_is_formatted() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let paths = sources(root);
    assert!(
        paths.len() > 100,
        "only {} file(s) listed under {} -- has the listing gone wrong?",
        paths.len(),
        root.display(),
    );

    let options = FormatOptions::default();
    let mut report = Vec::new();
    for relative in &paths {
        let written =
            fs::read_to_string(root.join(relative)).expect("a listed source should be readable");
        let formatted = try_fmt_file(&written, &options)
            .unwrap_or_else(|error| panic!("{}: {error}", relative.display()));
        if formatted != written {
            report.push(format!(
                "{} differs from line {}",
                relative.display(),
                first_difference(&written, &formatted),
            ));
        }
    }
    assert!(
        report.is_empty(),
        "these are not written the way `maudfmt` writes them; run `maudfmt` on each:\n{}",
        report.join("\n"),
    );
}
