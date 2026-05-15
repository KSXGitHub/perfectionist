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
