//! Write the unified diff [`super::try_apply`] reads.
//!
//! `git diff --no-index` writes it, rather than this module. A hunk
//! header carries the enclosing item's line, which git picks by a
//! heuristic of its own, and the format this has to land in is
//! defined as git's.

use crate::TempDir;
use command_extra::CommandExtra;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, str};

/// The unified diff from `original` to `fixed`, naming `name` in its
/// header lines. Two texts that agree have an empty diff.
pub(crate) fn unified(name: &str, original: &str, fixed: &str) -> String {
    assert!(
        original.ends_with('\n') && fixed.ends_with('\n'),
        "{name}: a fixture and its fixed form each end in a newline",
    );
    let temp = TempDir::new().expect("create a dir to diff in");
    let original = write(temp.path(), "original", name, original);
    let fixed = write(temp.path(), "fixed", name, fixed);
    let output = Command::new("git")
        .with_arg("diff")
        .with_arg("--no-index")
        .with_arg("--unified=3")
        .with_arg("--")
        .with_arg(&original)
        .with_arg(&fixed)
        .output()
        .expect("run `git diff --no-index`");
    // git answers 0 where the two files agree and 1 where they differ.
    assert!(
        matches!(output.status.code(), Some(0 | 1)),
        "`git diff --no-index` failed on {name}: {}",
        str::from_utf8(&output.stderr).unwrap_or("its stderr is not UTF-8"),
    );
    let diff = String::from_utf8(output.stdout).expect("`git diff` writes UTF-8 for a UTF-8 input");
    headers(name, &diff)
}

/// `text` in a directory of its own under `temp`, named `name` so
/// that the two sides of the diff differ only in their directory.
fn write(temp: &Path, side: &str, name: &str, text: &str) -> PathBuf {
    let dir = temp.join(side);
    fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("create {}: {error}", dir.display()));
    let file = dir.join(name);
    fs::write(&file, text).unwrap_or_else(|error| panic!("write {}: {error}", file.display()));
    file
}

/// `diff` with the pair of header lines [`super::try_apply`] skips, in
/// place of the paths git diffed and the `diff --git` line above them.
fn headers(name: &str, diff: &str) -> String {
    let mut lines = diff
        .split_inclusive('\n')
        .skip_while(|line| !line.starts_with("--- "));
    if lines.next().is_none() {
        return String::new();
    }
    lines
        .next()
        .expect("git writes a `+++` line under a `---` line");
    let mut text = format!("--- a/{name}\n+++ b/{name}\n");
    text.extend(lines);
    text
}

#[cfg(test)]
mod tests;
