//! Every maud template is written the way `maudfmt` writes it.

use command_extra::CommandExtra;
use into_sorted::IntoSorted;
use maudfmt::{FormatOptions, try_fmt_file};
use pipe_trait::Pipe;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml::{Table, Value};

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

/// Each directory holding a `rustfmt.toml`, against whether that file
/// switches formatting off.
fn rustfmt_configs(root: &Path, listed: &[&Path]) -> BTreeMap<PathBuf, bool> {
    listed
        .iter()
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name == "rustfmt.toml" || name == ".rustfmt.toml")
        })
        .map(|path| {
            let disabled = root
                .join(path)
                .pipe(fs::read_to_string)
                .expect("a listed rustfmt.toml should be readable")
                .parse::<Table>()
                .expect("a rustfmt.toml should hold valid TOML")
                .get("disable_all_formatting")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            (
                path.parent().unwrap_or(Path::new("")).to_path_buf(),
                disabled,
            )
        })
        .collect()
}

/// Whether rustfmt leaves `path` alone, by the nearest `rustfmt.toml`
/// above it -- which is the one rustfmt itself reads.
fn rustfmt_skips(path: &Path, configs: &BTreeMap<PathBuf, bool>) -> bool {
    path.ancestors()
        .skip(1)
        .find_map(|directory| configs.get(directory))
        .copied()
        .unwrap_or(false)
}

/// Every `.rs` file the repository at `root` counts as its own, tracked
/// or not, minus the ones rustfmt is switched off for. `try_fmt_file`
/// parses a whole file, and those are the files not meant to parse.
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
            "*rustfmt.toml",
        ],
    )
    .pipe(String::from_utf8)
    .expect("`git ls-files` produced non-UTF-8 output");
    let listed: Vec<&Path> = listing
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .map(Path::new)
        .collect();
    let configs = rustfmt_configs(root, &listed);
    listed
        .iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter(|path| !rustfmt_skips(path, &configs))
        .map(|path| path.to_path_buf())
        .collect::<Vec<_>>()
        .into_sorted()
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
        let written = root
            .join(relative)
            .pipe(fs::read_to_string)
            .expect("a listed source should be readable");
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
