//! `commit-msg` and `pre-push` git-hook entry points. Both reduce
//! their input to one or more (version-literal, snapshot) pairs
//! and feed them through `contract::verify`.

use std::fs;
use std::io::{self, BufRead};
use std::path::Path;

use super::contract::{Source, verify};
use super::error::RuntimeError;
use super::git::git_capture;
use super::version_literal::is_version_literal;

pub(crate) fn commit_msg(root: &Path, msg_file: &Path) -> Result<(), RuntimeError> {
    let content = fs::read_to_string(msg_file)
        .map_err(|err| RuntimeError::ReadMsgFile(msg_file.to_owned(), err))?;
    let comment = git_comment_char(root);
    let Some(subject) = extract_release_subject(&content, comment)? else {
        return Ok(());
    };
    verify(root, subject, &Source::Cached)
}

/// Read the user's effective `core.commentChar`, falling back to `#`
/// when the key is unset, set to `auto` (Git 2.45+'s adaptive mode),
/// or otherwise not a single character. The fallback matches Git's
/// own default and the literal we accept in the shell-side
/// pre-filter.
fn git_comment_char(root: &Path) -> char {
    let Ok(value) = git_capture(root, ["config", "--get", "core.commentChar"]) else {
        return '#';
    };
    let trimmed = value.trim_end_matches('\n');
    let mut chars = trimmed.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c,
        _ => '#',
    }
}

/// The body of Git's default "scissors" line — everything after the
/// comment char and its following space. `git commit --verbose`
/// appends the staged diff below a line of the form
/// `"<commentChar> {SCISSORS_BODY}"`, and Git strips that line and
/// everything after it before creating the commit. The hook must do
/// the same: the diff's lines aren't comment-prefixed, so without
/// this cut a release-shaped subject followed by a `-v` diff looks
/// like a forbidden commit body and trips `MessageHasExtraContent`.
const SCISSORS_BODY: &str = "------------------------ >8 ------------------------";

/// Parse the commit-message file's effective content (after cutting at
/// the verbose-diff scissors line and stripping `comment`-prefixed
/// lines and leading blanks). Returns:
///
/// * `Ok(Some(subject))` when the message is a release-shaped one
///   (`X.Y.Z(-<suffix>)?` only, no body).
/// * `Ok(None)` when the message is empty or doesn't look like a
///   release — the hook bows out and lets git proceed.
/// * `Err(MessageHasExtraContent)` when a release-shaped subject is
///   followed by additional body lines, which the contract forbids.
pub(crate) fn extract_release_subject(
    content: &str,
    comment: char,
) -> Result<Option<&str>, RuntimeError> {
    let scissors = format!("{comment} {SCISSORS_BODY}");
    let effective: Vec<&str> = content
        .lines()
        .take_while(|line| line.trim_end() != scissors)
        .filter(|line| !line.starts_with(comment))
        .map(str::trim_end)
        .skip_while(|line| line.is_empty())
        .collect();
    let subject = match effective.first() {
        Some(first) => *first,
        None => return Ok(None),
    };
    if !is_version_literal(subject) {
        return Ok(None);
    }
    if effective.iter().skip(1).any(|line| !line.is_empty()) {
        return Err(RuntimeError::MessageHasExtraContent(subject.to_owned()));
    }
    Ok(Some(subject))
}

pub(crate) fn pre_push(root: &Path) -> Result<(), RuntimeError> {
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
        if local_sha.chars().all(|char| char == '0') {
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
