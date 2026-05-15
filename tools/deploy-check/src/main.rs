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

fn run(Cli { root, command }: Cli) -> Result<(), RuntimeError> {
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

fn verify(root: &Path, version: &str, source: &Source) -> Result<(), RuntimeError> {
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
/// the before/after snapshots.
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
fn assert_version_only_diff(
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
    Ok(())
}

// ---------------------------------------------------------------------------
// Hook entry points.
// ---------------------------------------------------------------------------

fn commit_msg(root: &Path, msg_file: &Path) -> Result<(), RuntimeError> {
    let content = fs::read_to_string(msg_file)
        .map_err(|err| RuntimeError::ReadMsgFile(msg_file.to_owned(), err))?;
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
        return Err(RuntimeError::MessageHasExtraContent(subject.to_owned()));
    }
    verify(root, subject, &Source::Cached)
}

fn pre_push(root: &Path) -> Result<(), RuntimeError> {
    let stdin = io::stdin();
    let mut failed = false;
    for line in stdin.lock().lines() {
        let line = line.map_err(RuntimeError::ReadStdin)?;
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
        Err(RuntimeError::PrePushFailed)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Version-literal grammar: `<digits>.<digits>.<digits>(-<suffix>)?`,
// where `<suffix>` is non-empty and contains no whitespace.
//
// Each `take_*` peels a recognised prefix off the front of `input`
// and returns the remainder, per the parser-combinator convention
// in `planned-rules/IMPLEMENTATION_CONVENTIONS.md`.
// ---------------------------------------------------------------------------

fn is_version_literal(input: &str) -> bool {
    parse_version_literal(input).is_some()
}

fn parse_version_literal(input: &str) -> Option<()> {
    let (_, rest) = take_digits(input)?;
    let rest = rest.strip_prefix('.')?;
    let (_, rest) = take_digits(rest)?;
    let rest = rest.strip_prefix('.')?;
    let (_, rest) = take_digits(rest)?;
    if rest.is_empty() {
        return Some(());
    }
    let suffix = rest.strip_prefix('-')?;
    (!suffix.is_empty() && !suffix.chars().any(char::is_whitespace)).then_some(())
}

/// Take a non-empty run of ASCII digits from the front of `input`,
/// returning `(digits, rest)`.
fn take_digits(input: &str) -> Option<(&str, &str)> {
    let end = input.bytes().take_while(|b| b.is_ascii_digit()).count();
    (end > 0).then(|| input.split_at(end))
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

fn parse_cargo_toml_version(toml: &str) -> Result<String, RuntimeError> {
    toml.pipe(toml::from_str::<CargoTomlFile>)
        .map_err(RuntimeError::ParseCargoToml)?
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

fn parse_cargo_lock_version(toml: &str) -> Result<String, RuntimeError> {
    let lock: CargoLockFile = toml
        .pipe(toml::from_str)
        .map_err(RuntimeError::ParseCargoLock)?;
    let mut matches = lock.package.into_iter().filter(|p| p.name == PACKAGE_NAME);
    let first = matches.next().ok_or(RuntimeError::NoLockPackageEntry)?;
    if matches.next().is_some() {
        return Err(RuntimeError::DuplicateLockPackageEntry);
    }
    Ok(first.version)
}

// ---------------------------------------------------------------------------
// Plumbing.
// ---------------------------------------------------------------------------

fn git_capture<I, S>(root: &Path, args: I) -> Result<String, RuntimeError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = "git"
        .pipe(Command::new)
        .with_current_dir(root)
        .with_args(args)
        .output()
        .map_err(RuntimeError::SpawnGit)?;
    if !output.status.success() {
        return Err(RuntimeError::GitFailed {
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    String::from_utf8(output.stdout).map_err(|_| RuntimeError::GitNonUtf8)
}

mod error;
use error::RuntimeError;

#[cfg(test)]
mod tests;
