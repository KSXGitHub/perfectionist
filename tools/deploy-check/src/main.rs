//! Enforce the version-bump contract: a release commit (and its tag)
//! must satisfy
//!
//! 1. The tag is `X.Y.Z` or `X.Y.Z-<suffix>`.
//! 2. The commit message equals the tag exactly.
//! 3. The `[package].version` field in `Cargo.toml` equals the tag.
//! 4. The `perfectionist` package's `version` field in `Cargo.lock`
//!    equals the tag.
//! 5. The commit's diff against its parent modifies *only* `Cargo.toml`
//!    and `Cargo.lock`, and *only* on the two `version` lines above —
//!    no other line in either file is altered.
//!
//! Three entry points use the same check:
//!
//! * `verify <version>` — deploy CI (`HEAD` is the tagged commit).
//! * `commit-msg <file>` — `commit-msg` git hook (index vs. `HEAD`,
//!   when the typed message looks like a version literal).
//! * `pre-push` — `pre-push` git hook (each tag-ref update fed on
//!   stdin).
//!
//! When any of those fires on a release-shaped operation, this tool
//! exits non-zero so the operation aborts.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Parser, Subcommand};
use command_extra::CommandExtra;
use derive_more::Display;
use pipe_trait::Pipe;
use serde::Deserialize;

const PACKAGE_NAME: &str = "perfectionist";

#[derive(Parser)]
#[clap(about = "Validate the version-bump contract for deploy CI and git hooks")]
struct Cli {
    #[clap(help = "The root of the repository")]
    root: PathBuf,
    #[clap(subcommand)]
    command: Sub,
}

#[derive(Subcommand)]
enum Sub {
    #[clap(about = "Verify a commit against the version-bump contract")]
    Verify {
        #[clap(help = "Version literal to validate against (e.g. the tag name)")]
        version: String,
        #[clap(
            long,
            help = "Revision to verify (default: HEAD)",
            conflicts_with = "cached"
        )]
        commit: Option<String>,
        #[clap(
            long,
            help = "Verify the staged index against HEAD instead of a real commit"
        )]
        cached: bool,
    },
    #[clap(about = "commit-msg git-hook entry point")]
    CommitMsg {
        #[clap(help = "Path to the commit-message file passed by git")]
        msg_file: PathBuf,
    },
    #[clap(about = "pre-push git-hook entry point (reads ref updates from stdin)")]
    PrePush,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("deploy-check: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(Cli { root, command }: Cli) -> Result<(), Error> {
    match command {
        Sub::Verify {
            version,
            commit,
            cached,
        } => {
            let source = if cached {
                Source::Cached
            } else {
                Source::Commit(commit.unwrap_or_else(|| "HEAD".into()))
            };
            verify(&root, &version, &source)
        }
        Sub::CommitMsg { msg_file } => commit_msg(&root, &msg_file),
        Sub::PrePush => pre_push(&root),
    }
}

// ---------------------------------------------------------------------------
// Contract verification.
// ---------------------------------------------------------------------------

/// Whether we are checking a real commit (`Commit(rev)`) or the staged
/// index ahead of an in-progress commit (`Cached`).
enum Source {
    Commit(String),
    Cached,
}

impl Source {
    /// Read `file` either before (parent / HEAD) or after (commit /
    /// index) the change.
    fn read_file(&self, root: &Path, file: &str, before: bool) -> Result<String, Error> {
        let target = match (self, before) {
            (Source::Commit(rev), true) => format!("{rev}^:{file}"),
            (Source::Commit(rev), false) => format!("{rev}:{file}"),
            (Source::Cached, true) => format!("HEAD:{file}"),
            (Source::Cached, false) => format!(":{file}"),
        };
        git_capture(root, ["show", &target])
    }

    /// List the files changed by this source.
    fn changed_files(&self, root: &Path) -> Result<Vec<String>, Error> {
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

fn verify(root: &Path, version: &str, source: &Source) -> Result<(), Error> {
    if !is_version_literal(version) {
        return Err(Error::BadVersionLiteral(version.to_owned()));
    }

    if let Source::Commit(rev) = source {
        let msg = git_capture(root, ["log", "-1", "--format=%B", rev])?;
        let trimmed = msg.trim_end_matches('\n');
        if trimmed != version {
            return Err(Error::CommitMessageMismatch {
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
        return Err(Error::WrongFileSet(files));
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
/// the before/after snapshots.
fn verify_version_bump_file(
    source: &Source,
    root: &Path,
    version: &str,
    file: &str,
    parse_version: impl Fn(&str) -> Result<String, Error>,
) -> Result<(), Error> {
    let before = source.read_file(root, file, true)?;
    let after = source.read_file(root, file, false)?;
    let before_ver = parse_version(&before)?;
    let after_ver = parse_version(&after)?;
    if after_ver != version {
        return Err(Error::VersionMismatch {
            file: file.to_owned(),
            expected: version.to_owned(),
            got: after_ver,
        });
    }
    if before_ver == after_ver {
        return Err(Error::NoVersionChange(file.to_owned()));
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
fn assert_version_only_diff(
    file: &str,
    before: &str,
    after: &str,
    before_ver: &str,
    after_ver: &str,
) -> Result<(), Error> {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();
    if before_lines.len() != after_lines.len() {
        return Err(Error::LineCountChanged(file.to_owned()));
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
        .ok_or_else(|| Error::NoVersionChange(file.to_owned()))?;
    if *b != expected_before {
        return Err(Error::UnexpectedBeforeLine {
            file: file.to_owned(),
            line: idx + 1,
            expected: expected_before,
            got: (*b).to_owned(),
        });
    }
    if *a != expected_after {
        return Err(Error::UnexpectedAfterLine {
            file: file.to_owned(),
            line: idx + 1,
            expected: expected_after,
            got: (*a).to_owned(),
        });
    }
    if let Some((extra_idx, (extra_b, extra_a))) = diffs.next() {
        return Err(Error::ExtraLineChanged {
            file: file.to_owned(),
            line: extra_idx + 1,
            before: (*extra_b).to_owned(),
            after: (*extra_a).to_owned(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Hook entry points.
// ---------------------------------------------------------------------------

fn commit_msg(root: &Path, msg_file: &Path) -> Result<(), Error> {
    let content =
        fs::read_to_string(msg_file).map_err(|err| Error::ReadMsgFile(msg_file.to_owned(), err))?;
    let effective: Vec<&str> = content
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(str::trim_end)
        .skip_while(|line| line.is_empty())
        .collect();
    let subject = match effective.first() {
        Some(first) => *first,
        None => return Ok(()),
    };
    if !is_version_literal(subject) {
        return Ok(());
    }
    if effective.iter().skip(1).any(|line| !line.is_empty()) {
        return Err(Error::MessageHasExtraContent(subject.to_owned()));
    }
    verify(root, subject, &Source::Cached)
}

fn pre_push(root: &Path) -> Result<(), Error> {
    let stdin = io::stdin();
    let mut failed = false;
    for line in stdin.lock().lines() {
        let line = line.map_err(Error::ReadStdin)?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }
        let local_ref = parts[0];
        let local_sha = parts[1];
        let tag_name = match local_ref.strip_prefix("refs/tags/") {
            Some(name) => name,
            None => continue,
        };
        if !is_version_literal(tag_name) {
            continue;
        }
        if local_sha.chars().all(|c| c == '0') {
            // Tag deletion — nothing to verify.
            continue;
        }
        // Dereference annotated tags to their commit before running
        // the diff-based checks.
        let commit = git_capture(root, ["rev-parse", &format!("{local_sha}^{{commit}}")])?
            .trim()
            .to_owned();
        if let Err(err) = verify(root, tag_name, &Source::Commit(commit)) {
            eprintln!("deploy-check: tag {tag_name}: {err}");
            failed = true;
        }
    }
    if failed {
        Err(Error::PrePushFailed)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Version-literal grammar: `<digits>.<digits>.<digits>(-<suffix>)?`,
// where `<suffix>` is non-empty and contains no whitespace.
// ---------------------------------------------------------------------------

fn is_version_literal(s: &str) -> bool {
    let mut chars = s.chars().peekable();
    if !take_digits(&mut chars) {
        return false;
    }
    if chars.next() != Some('.') {
        return false;
    }
    if !take_digits(&mut chars) {
        return false;
    }
    if chars.next() != Some('.') {
        return false;
    }
    if !take_digits(&mut chars) {
        return false;
    }
    match chars.next() {
        None => true,
        Some('-') => {
            let suffix: String = chars.collect();
            !suffix.is_empty() && !suffix.chars().any(char::is_whitespace)
        }
        _ => false,
    }
}

fn take_digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    let mut count = 0usize;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            chars.next();
            count += 1;
        } else {
            break;
        }
    }
    count > 0
}

// ---------------------------------------------------------------------------
// Manifest / lockfile parsing.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CargoTomlFile {
    package: CargoTomlPackage,
}

#[derive(Deserialize)]
struct CargoTomlPackage {
    version: String,
}

fn parse_cargo_toml_version(content: &str) -> Result<String, Error> {
    content
        .pipe(toml::from_str::<CargoTomlFile>)
        .map_err(|err| err.to_string())
        .map_err(Error::ParseCargoToml)?
        .package
        .version
        .pipe(Ok)
}

#[derive(Deserialize)]
struct CargoLockFile {
    #[serde(default)]
    package: Vec<CargoLockPackage>,
}

#[derive(Deserialize)]
struct CargoLockPackage {
    name: String,
    version: String,
}

fn parse_cargo_lock_version(content: &str) -> Result<String, Error> {
    let lock: CargoLockFile =
        toml::from_str(content).map_err(|err| Error::ParseCargoLock(err.to_string()))?;
    let mut matches = lock.package.into_iter().filter(|p| p.name == PACKAGE_NAME);
    let first = matches.next().ok_or(Error::NoLockPackageEntry)?;
    if matches.next().is_some() {
        return Err(Error::DuplicateLockPackageEntry);
    }
    Ok(first.version)
}

// ---------------------------------------------------------------------------
// Plumbing.
// ---------------------------------------------------------------------------

fn git_capture<I, S>(root: &Path, args: I) -> Result<String, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = "git".pipe(Command::new).with_current_dir(root);
    for arg in args {
        command = command.with_arg(arg);
    }
    let output = command.output().map_err(Error::SpawnGit)?;
    if !output.status.success() {
        return Err(Error::GitFailed {
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    String::from_utf8(output.stdout).map_err(|_| Error::GitNonUtf8)
}

#[derive(Display, Debug)]
enum Error {
    #[display("failed to spawn git: {_0}")]
    SpawnGit(io::Error),
    #[display("git exited with status {status:?}: {stderr}")]
    GitFailed { status: Option<i32>, stderr: String },
    #[display("git produced non-UTF-8 output")]
    GitNonUtf8,
    #[display("failed to read commit-message file {}: {_1}", _0.display())]
    ReadMsgFile(PathBuf, io::Error),
    #[display("failed to read stdin: {_0}")]
    ReadStdin(io::Error),
    #[display("version literal {_0:?} does not match `X.Y.Z` or `X.Y.Z-<suffix>`")]
    BadVersionLiteral(String),
    #[display("commit {rev} message {got:?} does not match the tag {expected:?}")]
    CommitMessageMismatch {
        rev: String,
        expected: String,
        got: String,
    },
    #[display(
        "version-bump commit must modify only Cargo.toml and Cargo.lock, but the diff touches: {_0:?}"
    )]
    WrongFileSet(Vec<String>),
    #[display("failed to parse Cargo.toml: {_0}")]
    ParseCargoToml(String),
    #[display("failed to parse Cargo.lock: {_0}")]
    ParseCargoLock(String),
    #[display("Cargo.lock has no [[package]] entry for `{PACKAGE_NAME}`")]
    NoLockPackageEntry,
    #[display("Cargo.lock has more than one [[package]] entry for `{PACKAGE_NAME}`")]
    DuplicateLockPackageEntry,
    #[display(
        "{file} encodes `{PACKAGE_NAME}` at version {got:?} but the tag/message is {expected:?}"
    )]
    VersionMismatch {
        file: String,
        expected: String,
        got: String,
    },
    #[display("{_0} has no version change in this diff")]
    NoVersionChange(String),
    #[display("{_0} changed line count — version-bump commits must keep it identical")]
    LineCountChanged(String),
    #[display(
        "{file}: line {line} differs from the expected before-image\n  expected: {expected}\n  actual:   {got}"
    )]
    UnexpectedBeforeLine {
        file: String,
        line: usize,
        expected: String,
        got: String,
    },
    #[display(
        "{file}: line {line} differs from the expected after-image\n  expected: {expected}\n  actual:   {got}"
    )]
    UnexpectedAfterLine {
        file: String,
        line: usize,
        expected: String,
        got: String,
    },
    #[display(
        "{file}: line {line} also changed — version-bump commits must touch only the version line\n  before: {before}\n  after:  {after}"
    )]
    ExtraLineChanged {
        file: String,
        line: usize,
        before: String,
        after: String,
    },
    #[display(
        "commit message starts with a version literal ({_0:?}) but also contains additional body lines — release commits must be the version literal only"
    )]
    MessageHasExtraContent(String),
    #[display("one or more tag updates fail the version-bump contract")]
    PrePushFailed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_literal_grammar() {
        for ok in ["0.0.0", "1.2.3", "0.0.0-rc.13", "10.20.30-beta.4+sha.abc"] {
            assert!(is_version_literal(ok), "{ok:?} should be accepted");
        }
        for bad in [
            "",
            "1.2",
            "1.2.3.4",
            "v1.2.3",
            "1.2.3-",
            "1.2.3 ",
            " 1.2.3",
            "1.2.3-rc 1",
            "1.a.0",
            "1.2.3+meta",
        ] {
            assert!(!is_version_literal(bad), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn cargo_toml_version_extracted_from_package_section() {
        let manifest = "\
[workspace]
members = [\"x\"]

[package]
name = \"perfectionist\"
version = \"0.0.0-rc.7\"
";
        assert_eq!(parse_cargo_toml_version(manifest).unwrap(), "0.0.0-rc.7");
    }

    #[test]
    fn cargo_lock_version_picks_perfectionist_entry() {
        let lock = "\
[[package]]
name = \"foo\"
version = \"1.0.0\"

[[package]]
name = \"perfectionist\"
version = \"0.0.0-rc.5\"
dependencies = [
 \"foo\",
]

[[package]]
name = \"bar\"
version = \"2.0.0\"
";
        assert_eq!(parse_cargo_lock_version(lock).unwrap(), "0.0.0-rc.5");
    }

    #[test]
    fn cargo_lock_rejects_duplicate_perfectionist_entries() {
        let lock = "\
[[package]]
name = \"perfectionist\"
version = \"0.0.0-rc.1\"

[[package]]
name = \"perfectionist\"
version = \"0.0.0-rc.2\"
";
        assert!(matches!(
            parse_cargo_lock_version(lock),
            Err(Error::DuplicateLockPackageEntry)
        ));
    }

    #[test]
    fn version_only_diff_accepts_a_pure_version_bump() {
        let before = "\
[package]
name = \"perfectionist\"
version = \"0.0.0-rc.6\"
";
        let after = "\
[package]
name = \"perfectionist\"
version = \"0.0.0-rc.7\"
";
        assert_version_only_diff("Cargo.toml", before, after, "0.0.0-rc.6", "0.0.0-rc.7").unwrap();
    }

    #[test]
    fn version_only_diff_rejects_collateral_edits() {
        let before = "\
[package]
name = \"perfectionist\"
version = \"0.0.0-rc.6\"
";
        let after = "\
[package]
name = \"renamed\"
version = \"0.0.0-rc.7\"
";
        // The first differing line is `name = ...`, which doesn't
        // match the expected version-line text — so the rejection
        // is `UnexpectedBeforeLine`, not `ExtraLineChanged`.
        assert!(matches!(
            assert_version_only_diff("Cargo.toml", before, after, "0.0.0-rc.6", "0.0.0-rc.7"),
            Err(Error::UnexpectedBeforeLine { .. })
        ));
    }

    #[test]
    fn version_only_diff_rejects_an_extra_changed_line_after_the_version() {
        let before = "\
[package]
version = \"0.0.0-rc.6\"
description = \"a\"
";
        let after = "\
[package]
version = \"0.0.0-rc.7\"
description = \"b\"
";
        assert!(matches!(
            assert_version_only_diff("Cargo.toml", before, after, "0.0.0-rc.6", "0.0.0-rc.7"),
            Err(Error::ExtraLineChanged { .. })
        ));
    }

    #[test]
    fn version_only_diff_rejects_line_count_mismatch() {
        let before = "version = \"0.0.0-rc.6\"\n";
        let after = "version = \"0.0.0-rc.7\"\nextra\n";
        assert!(matches!(
            assert_version_only_diff("Cargo.toml", before, after, "0.0.0-rc.6", "0.0.0-rc.7"),
            Err(Error::LineCountChanged(_))
        ));
    }

    #[test]
    fn version_only_diff_rejects_byte_identical_inputs() {
        let same = "version = \"0.0.0-rc.6\"\n";
        assert!(matches!(
            assert_version_only_diff("Cargo.toml", same, same, "0.0.0-rc.6", "0.0.0-rc.6"),
            Err(Error::NoVersionChange(_))
        ));
    }
}
