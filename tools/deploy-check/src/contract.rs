//! The version-bump contract and its enforcement.

use std::path::Path;

use pipe_trait::Pipe;

use super::error::RuntimeError;
use super::git::git_capture;
use super::manifest::{parse_cargo_lock_version, parse_cargo_toml_version};
use super::version_literal::is_version_literal;

/// Whether we are checking a real commit (`Commit(rev)`) or the staged
/// index ahead of an in-progress commit (`Cached`).
pub(crate) enum Source {
    Commit(String),
    Cached,
}

impl Source {
    /// Read `file` either before (parent / HEAD) or after (commit /
    /// index) the change.
    fn read_file(&self, root: &Path, file: &str, before: bool) -> Result<String, RuntimeError> {
        let target = match (self, before) {
            (Source::Commit(rev), true) => format!("{rev}^:{file}"),
            (Source::Commit(rev), false) => format!("{rev}:{file}"),
            (Source::Cached, true) => format!("HEAD:{file}"),
            (Source::Cached, false) => format!(":{file}"),
        };
        git_capture(root, ["show", &target])
    }

    /// List the files changed by this source.
    fn changed_files(&self, root: &Path) -> Result<Vec<String>, RuntimeError> {
        let stdout = match self {
            Source::Commit(rev) => {
                git_capture(root, ["diff", "--name-only", &format!("{rev}^..{rev}")])?
            }
            Source::Cached => git_capture(root, ["diff", "--cached", "--name-only", "HEAD"])?,
        };
        stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>()
            .pipe(Ok)
    }
}

pub(crate) fn verify(root: &Path, version: &str, source: &Source) -> Result<(), RuntimeError> {
    if !is_version_literal(version) {
        return Err(RuntimeError::BadVersionLiteral(version.to_owned()));
    }

    if let Source::Commit(rev) = source {
        let msg = git_capture(root, ["log", "-1", "--format=%B", rev])?;
        let trimmed = msg.trim_end_matches('\n');
        if trimmed != version {
            return Err(RuntimeError::CommitMessageMismatch {
                rev: rev.clone(),
                expected: version.to_owned(),
                got: trimmed.to_owned(),
            });
        }
    }

    let mut files = source.changed_files(root)?;
    files.sort();
    let expected_files = ["Cargo.lock", "Cargo.toml"];
    if files.iter().map(String::as_str).ne(expected_files) {
        return Err(RuntimeError::WrongFileSet(files));
    }

    verify_version_bump_file(
        source,
        root,
        version,
        "Cargo.toml",
        parse_cargo_toml_version,
    )?;
    verify_version_bump_file(
        source,
        root,
        version,
        "Cargo.lock",
        parse_cargo_lock_version,
    )?;

    Ok(())
}

/// Verify the contract for one of the two tracked files: the
/// package's version (read by `parse_version`) changes from one
/// release literal to another, and nothing else differs between
/// the before/after snapshots. The two versions only need to be
/// *different* — neither monotonicity nor SemVer ordering matters.
fn verify_version_bump_file(
    source: &Source,
    root: &Path,
    version: &str,
    file: &str,
    parse_version: impl Fn(&str) -> Result<String, RuntimeError>,
) -> Result<(), RuntimeError> {
    let before = source.read_file(root, file, true)?;
    let after = source.read_file(root, file, false)?;
    let before_ver = parse_version(&before)?;
    let after_ver = parse_version(&after)?;
    if after_ver != version {
        return Err(RuntimeError::VersionMismatch {
            file: file.to_owned(),
            expected: version.to_owned(),
            got: after_ver,
        });
    }
    if before_ver == after_ver {
        return Err(RuntimeError::NoVersionChange(file.to_owned()));
    }
    assert_version_only_diff(file, &before, &after, &before_ver, &after_ver)
}

/// Reject the snapshot pair unless `before` and `after` differ on
/// exactly one line, that line reads `version = "<before_ver>"`
/// before and `version = "<after_ver>"` after. The parser at the
/// caller has already confirmed that this package's version moved
/// from `before_ver` to `after_ver`, so if exactly one line differs
/// and its text matches the expected version literal, that line
/// must be the package's version line — no positional check needed.
pub(crate) fn assert_version_only_diff(
    file: &str,
    before: &str,
    after: &str,
    before_ver: &str,
    after_ver: &str,
) -> Result<(), RuntimeError> {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();
    if before_lines.len() != after_lines.len() {
        return Err(RuntimeError::LineCountChanged(file.to_owned()));
    }
    let expected_before = format!("version = \"{before_ver}\"");
    let expected_after = format!("version = \"{after_ver}\"");
    let mut diffs = before_lines
        .iter()
        .zip(&after_lines)
        .enumerate()
        .filter(|(_, (b, a))| b != a);
    let (idx, (b, a)) = diffs
        .next()
        .ok_or_else(|| RuntimeError::NoVersionChange(file.to_owned()))?;
    if *b != expected_before {
        return Err(RuntimeError::UnexpectedBeforeLine {
            file: file.to_owned(),
            line: idx + 1,
            expected: expected_before,
            got: (*b).to_owned(),
        });
    }
    if *a != expected_after {
        return Err(RuntimeError::UnexpectedAfterLine {
            file: file.to_owned(),
            line: idx + 1,
            expected: expected_after,
            got: (*a).to_owned(),
        });
    }
    if let Some((extra_idx, (extra_b, extra_a))) = diffs.next() {
        return Err(RuntimeError::ExtraLineChanged {
            file: file.to_owned(),
            line: extra_idx + 1,
            before: (*extra_b).to_owned(),
            after: (*extra_a).to_owned(),
        });
    }
    // Final byte-level sanity check: `str::lines()` smooths over
    // trailing-newline-at-EOF changes and conflates `\n` / `\r\n`,
    // so a release commit that also alters the version line's EOL
    // style or strips/adds the file's final newline can pass the
    // line-by-line check above. Synthesise what `after` should look
    // like by swapping the version line in `before`, then compare
    // byte-equal.
    let synthesised = before.replacen(&expected_before, &expected_after, 1);
    if synthesised != after {
        return Err(RuntimeError::NonVersionByteDiff(file.to_owned()));
    }
    Ok(())
}
