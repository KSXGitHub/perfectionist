//! The crate's single top-level error type. Each variant covers
//! one failure mode along the verify / commit-msg / pre-push paths,
//! and carries enough structured context that the
//! [`Display`](std::fmt::Display) impl produces an actionable message
//! at the CLI.

use super::PACKAGE_NAME;
use derive_more::Display;
use std::io;
use std::path::PathBuf;

#[derive(Display, Debug)]
pub(crate) enum RuntimeError {
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
    ParseCargoToml(toml::de::Error),
    #[display("failed to parse Cargo.lock: {_0}")]
    ParseCargoLock(toml::de::Error),
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
        "{_0}: byte-level difference outside the version line (trailing newline or end-of-line style differs); restore the file to its pre-bump byte-for-byte content except for the version line"
    )]
    NonVersionByteDiff(String),
    #[display(
        "commit message starts with a version literal ({_0:?}) but also contains additional body lines — release commits must be the version literal only"
    )]
    MessageHasExtraContent(String),
    #[display("one or more tag updates fail the version-bump contract")]
    PrePushFailed,
}
