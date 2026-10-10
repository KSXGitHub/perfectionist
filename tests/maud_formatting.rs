//! Every maud template is written the way `maudfmt` writes it.
//!
//! rustfmt does not enter a `{`-delimited macro invocation, so a
//! `maud::html!` body is beyond what `just fmt` settles.

use command_extra::CommandExtra;
use maudfmt::{FormatOptions, try_fmt_file};
use pipe_trait::Pipe;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Fixture trees, left out because `try_fmt_file` parses a whole file
/// and some of these are not meant to parse.
const UNSCANNED: &[&str] = &["ui", "ui-toml", "tests/fixtures"];

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

/// Every `.rs` file the repository at `root` counts as its own,
/// tracked or not, minus [`UNSCANNED`].
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
    // `--cached` and `--others` each list in order, but one runs after
    // the other.
    found.sort();
    found
}

/// The 1-based line number where `written` and `formatted` first differ.
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
