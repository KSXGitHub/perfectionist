//! `commit-msg`, `pre-push` and `reference-transaction` git-hook
//! entry points. Each reduces its input to the (version-literal,
//! snapshot) pairs it finds and feeds them through
//! `contract::verify`.

use super::contract::{Source, verify};
use super::error::RuntimeError;
use super::git::{git_capture, git_is_shallow, git_rev_present};
use super::version_literal::is_version_literal;
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;

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

/// The body of Git's default "scissors" line: everything after the
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
///   release, so the hook exits and lets git proceed.
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
    check_tag_updates(root, pushed_update)
}

/// The state git runs the `reference-transaction` hook with while the
/// refs are still unwritten. It is the only one whose non-zero exit
/// stops the update: `committed` and `aborted` name a transaction
/// that has already been decided, and git ignores the hook's exit
/// status for them.
const PREPARED: &str = "prepared";

/// The `reference-transaction` git-hook entry point, which is what
/// catches `git tag <version>` on a commit the contract rejects.
///
/// A ref transaction covers the refs a fetch writes as well as the ones
/// a `git tag` writes, and git hands the hook the same input either
/// way, so a fetched tag is judged exactly as a locally written one is.
/// One that fails the contract fails the whole fetch, so this prints
/// `git -c core.hooksPath= fetch` as the way to take it anyway.
pub(crate) fn reference_transaction(root: &Path, state: &str) -> Result<(), RuntimeError> {
    if state != PREPARED {
        return Ok(());
    }
    let outcome = check_tag_updates(root, transaction_update);
    if matches!(outcome, Err(RuntimeError::TagUpdatesFailed)) {
        eprintln!(
            "deploy-check: if this tag is arriving from a fetch rather than one you \
             are writing, `git -c core.hooksPath= fetch` takes it without this check.",
        );
    }
    outcome
}

/// Pick the ref and the object it will point at out of one line of
/// `pre-push` input: `<local-ref> <local-sha> <remote-ref> <remote-sha>`.
pub(crate) fn pushed_update(line: &str) -> Option<(&str, &str)> {
    let mut fields = line.split_whitespace();
    let local_ref = fields.next()?;
    let local_sha = fields.next()?;
    // The remote half decides nothing here, but a line without it is
    // not a ref update.
    fields.next()?;
    fields.next()?;
    Some((local_ref, local_sha))
}

/// Pick the ref and the object it will point at out of one line of
/// `reference-transaction` input: `<old-value> <new-value> <ref-name>`.
pub(crate) fn transaction_update(line: &str) -> Option<(&str, &str)> {
    let mut fields = line.split_whitespace();
    // The old value decides nothing here: the contract is read off the
    // commit the ref is about to point at.
    fields.next()?;
    let new_object = fields.next()?;
    let ref_name = fields.next()?;
    Some((ref_name, new_object))
}

/// Verify every release-shaped tag among the ref updates on stdin.
///
/// `parse` reads one input line as `(ref name, new object)`; the layout
/// of that line is all that differs between `pre-push` and
/// `reference-transaction`.
fn check_tag_updates(
    root: &Path,
    parse: impl Fn(&str) -> Option<(&str, &str)>,
) -> Result<(), RuntimeError> {
    let stdin = io::stdin();
    let mut failed = false;
    for line in stdin.lock().lines() {
        let line = line.map_err(RuntimeError::ReadStdin)?;
        let Some((ref_name, new_object)) = parse(&line) else {
            continue;
        };
        let Some(tag_name) = ref_name.strip_prefix("refs/tags/") else {
            continue;
        };
        if !is_version_literal(tag_name) {
            continue;
        }
        if new_object.chars().all(|char| char == '0') {
            // Tag deletion, so nothing to verify.
            continue;
        }
        if !check_tag(root, tag_name, new_object)? {
            failed = true;
        }
    }
    if failed {
        Err(RuntimeError::TagUpdatesFailed)
    } else {
        Ok(())
    }
}

/// Whether one release-shaped tag holds the contract, printing any
/// failure on stderr.
fn check_tag(root: &Path, tag_name: &str, new_object: &str) -> Result<bool, RuntimeError> {
    // Dereference annotated tags to their commit before running the
    // diff-based checks.
    let commit = git_capture(root, ["rev-parse", &format!("{new_object}^{{commit}}")])?
        .trim()
        .to_owned();
    if !git_rev_present(root, &format!("{commit}^"))? {
        return check_parentless_tag(root, tag_name, &commit);
    }
    if let Err(err) = verify(root, tag_name, &Source::Commit(commit)) {
        eprintln!("deploy-check: tag {tag_name}: {err}");
        return Ok(false);
    }
    Ok(true)
}

/// Whether a release-shaped tag holds when its commit's parent is
/// absent from the object database.
///
/// Every check in the contract compares the commit against its parent,
/// so without one there is no before-image to read.
///
/// A shallow clone is the case that arises in practice: the ancestors
/// of its boundary commits are absent, so a fetch landing a release
/// tag on one would otherwise be refused for a contract this checkout
/// cannot read. Such a tag is left to deploy CI, which fetches the
/// tagged commit's parent and so can read the contract.
///
/// In a complete clone the only commit without a parent is the root
/// commit, where the contract genuinely cannot hold.
fn check_parentless_tag(root: &Path, tag_name: &str, commit: &str) -> Result<bool, RuntimeError> {
    if git_is_shallow(root)? {
        eprintln!(
            "deploy-check: tag {tag_name}: not checked, {commit} is at this \
             checkout's shallow boundary so its parent is absent",
        );
        return Ok(true);
    }
    eprintln!(
        "deploy-check: tag {tag_name}: {commit} has no parent, so there is no \
         before-image to check a version bump against",
    );
    Ok(false)
}
