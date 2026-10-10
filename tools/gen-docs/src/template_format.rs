//! One check on the `html!` templates: a block closes at the indent it
//! opened at.
//!
//! rustfmt does not enter a macro body, so a template whose `{` and `}`
//! disagree compiles, renders correctly and passes every other step.
//! It reads wrong, though, because a child indented level with the
//! attributes above it looks like another attribute. That had already
//! happened twice before anything checked for it.

use std::fs;
use std::path::{Path, PathBuf};

/// Where a block opened: its line number, and the indent to close at.
type Opened = (usize, usize);

/// Every `.rs` under `src/` holding a template, taken from the tree so
/// that a new one is covered without being named here.
fn template_sources(dir: &Path, found: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("a directory entry should be readable").path();
        if path.is_dir() {
            template_sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && fs::read_to_string(&path).is_ok_and(|source| source.contains("html! {"))
        {
            found.push(path);
        }
    }
}

/// Pair each block-opening `{` with the `}` that closes it, and report
/// the pairs whose indents disagree.
///
/// Only a line *ending* in `{` opens, and only a `}` *leading* a line
/// closes, which is what keeps a brace pair written on one line out of
/// it. A line doing both — `} @else {` — is read in that order.
fn misaligned(source: &str) -> Vec<String> {
    let mut open: Vec<Opened> = Vec::new();
    let mut found = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let indent = line.len() - trimmed.len();
        let closers = trimmed
            .chars()
            .take_while(|character| *character == '}')
            .count();
        for _ in 0..closers {
            if let Some((opened_on, opened_at)) = open.pop()
                && opened_at != indent
            {
                found.push(format!(
                    "line {opened_on} opens at column {opened_at}, \
                     but line {} closes it at column {indent}",
                    index + 1,
                ));
            }
        }
        if trimmed.ends_with('{') {
            open.push((index + 1, indent));
        }
    }
    found
}

#[cfg(test)]
mod tests;
