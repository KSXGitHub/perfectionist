//! Forge-URL matcher: locate the ref segment of a hosted-repository
//! URL and decide whether the URL shape already proves the ref is
//! mutable.
//!
//! The matcher is parser-combinator-style per
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`. A URL is split into
//! its path segments once ([`segments`]); each per-kind matcher then
//! threads the segment slice through [`take_capture`] (consume one
//! segment, keep its text + offset), [`take_literal`] (require a fixed
//! segment such as `src`), and [`take_literal_any`] (require one of a
//! fixed set, such as the verb `blob`/`tree`/`raw`). The five matchers
//! are dispatched by [`locate_ref`] from the host's resolved
//! [`ForgeKind`].
//!
//! Whether the located ref is *acceptable* (a SHA) is a separate step,
//! [`ref_problem`]; this module only finds the ref and reports what
//! the URL shape alone says about it.

use crate::rules::unpinned_repo_ref::config::ForgeKind;

/// What the URL shape says the ref must be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefOutcome {
    /// The ref is only acceptable if it looks like a commit SHA. The
    /// normal case, and gitea's `/src/commit/` form.
    MustBeSha,
    /// The URL path itself names the ref kind as mutable (gitea's
    /// `/src/branch/` or `/src/tag/`); the ref text is irrelevant.
    KnownMutable(MutableKind),
}

/// The mutable ref kind a gitea-style URL names in its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MutableKind {
    Branch,
    Tag,
}

/// Why a located ref is rejected. Drives the diagnostic message; the
/// `Branch` / `Tag` distinction is only available for gitea-style URLs
/// (which name the kind in the path), so a non-gitea unpinned ref is
/// always [`RefProblem::NotSha`] regardless of whether it happens to
/// be a branch or a tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RefProblem {
    Branch,
    Tag,
    NotSha,
}

/// A located ref: its text, its byte offset within the URL (so the
/// caller can build a source span), and what the URL shape requires
/// of it.
pub(super) struct RefLocation<'a> {
    pub(super) text: &'a str,
    pub(super) offset: usize,
    pub(super) outcome: RefOutcome,
}

/// One path segment: its byte offset within the path slice and its
/// text (delimiting slashes excluded). The text's lifetime is the
/// URL's, kept distinct from the segment slice's own borrow so a
/// captured segment can outlive the [`Vec`] that held it.
type Segment<'a> = (usize, &'a str);

/// Extract the host of `url` (no scheme, no userinfo, no port, no
/// path), or `None` if `url` has no `://`. Used by the pass to look
/// the host up in the configured forge table before dispatching here.
///
/// Follows the RFC 3986 authority shape `[userinfo "@"] host [":"
/// port]`: the authority runs to the first `/`, `?`, or `#`; userinfo
/// (everything through the last `@`) and a trailing `:port` are
/// stripped. Without the userinfo strip, a `user:pass@host` URL would
/// yield `user` and silently fail the host lookup.
pub(crate) fn url_host(url: &str) -> Option<&str> {
    let scheme_end = url.find("://")? + 3;
    let rest = &url[scheme_end..];
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let after_userinfo = match authority.rfind('@') {
        Some(at_index) => &authority[at_index + 1..],
        None => authority,
    };
    let host_end = after_userinfo.find(':').unwrap_or(after_userinfo.len());
    Some(&after_userinfo[..host_end])
}

/// Locate the ref of `url`, interpreted under `kind`'s URL shape.
/// `None` when the URL doesn't match the kind's file-reference shape
/// (e.g. a `/commit/<sha>` or `/compare/...` URL, which this rule
/// leaves to `commit-id-length`).
pub(super) fn locate_ref(url: &str, kind: ForgeKind) -> Option<RefLocation<'_>> {
    let (path_offset, path) = split_path(url)?;
    let segments = segments(path);
    let ((segment_offset, text), outcome) = match kind {
        ForgeKind::Github => match_github(&segments),
        ForgeKind::Gitlab => match_gitlab(&segments),
        ForgeKind::Bitbucket => match_bitbucket(&segments),
        ForgeKind::Gitea => match_gitea(&segments),
        ForgeKind::Sourcehut => match_sourcehut(&segments),
    }?;
    Some(RefLocation {
        text,
        offset: path_offset + segment_offset,
        outcome,
    })
}

/// Classify a located ref. `None` means the ref is acceptable (a
/// commit SHA, or a release-shaped tag when configured);
/// `Some(problem)` is a violation.
pub(crate) fn ref_problem(
    text: &str,
    outcome: RefOutcome,
    sha_recognition_length: usize,
    allow_release_tags: bool,
) -> Option<RefProblem> {
    match outcome {
        RefOutcome::KnownMutable(MutableKind::Branch) => Some(RefProblem::Branch),
        RefOutcome::KnownMutable(MutableKind::Tag) => {
            if allow_release_tags && is_release_tag_ref(text) {
                None
            } else {
                Some(RefProblem::Tag)
            }
        }
        RefOutcome::MustBeSha => {
            if is_commit_sha(text, sha_recognition_length)
                || (allow_release_tags && is_release_tag_ref(text))
            {
                None
            } else {
                Some(RefProblem::NotSha)
            }
        }
    }
}

/// Whether `text` is a pure-hex run of at least `sha_recognition_length`
/// characters — the rule's recognition test for a commit SHA.
fn is_commit_sha(text: &str, sha_recognition_length: usize) -> bool {
    text.len() >= sha_recognition_length
        && !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Whether `text` looks like a release tag (`1.2.3`, `v1.2.3`, or
/// either form with a non-empty `-suffix`). This intentionally avoids a
/// regex dependency because the lint is compiled from source by users.
fn is_release_tag_ref(text: &str) -> bool {
    let text = text.strip_prefix('v').unwrap_or(text);
    let (version, suffix) = text
        .split_once('-')
        .map_or((text, None), |(version, suffix)| (version, Some(suffix)));
    if suffix.is_some_and(str::is_empty) {
        return false;
    }

    let mut parts = version.split('.');
    let [Some(major), Some(minor), Some(patch), None] =
        [parts.next(), parts.next(), parts.next(), parts.next()]
    else {
        return false;
    };
    [major, minor, patch]
        .into_iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Split `url` into the byte offset of its path and the path text
/// itself (query string and fragment removed). `None` when there is
/// no `://` or no path after the host.
fn split_path(url: &str) -> Option<(usize, &str)> {
    let scheme_end = url.find("://")? + 3;
    let rest = &url[scheme_end..];
    let host_len = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    if host_len == rest.len() {
        return None;
    }
    let path_offset = scheme_end + host_len;
    let path = &url[path_offset..];
    let path_len = path.find(['?', '#']).unwrap_or(path.len());
    Some((path_offset, &path[..path_len]))
}

/// Split a path into its non-empty segments, each tagged with its byte
/// offset within the path. Leading, trailing, and doubled slashes
/// produce no empty segments.
fn segments(path: &str) -> Vec<Segment<'_>> {
    let bytes = path.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'/' {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && bytes[index] != b'/' {
            index += 1;
        }
        out.push((start, &path[start..index]));
    }
    out
}

/// Consume one segment, returning it and the remaining segments.
fn take_capture<'s, 'a>(segments: &'s [Segment<'a>]) -> Option<(Segment<'a>, &'s [Segment<'a>])> {
    let (first, rest) = segments.split_first()?;
    Some((*first, rest))
}

/// Consume one segment, requiring its text to equal `literal`.
fn take_literal<'s, 'a>(segments: &'s [Segment<'a>], literal: &str) -> Option<&'s [Segment<'a>]> {
    let ((_, value), rest) = segments.split_first()?;
    (*value == literal).then_some(rest)
}

/// Consume one segment, requiring its text to be one of `literals`;
/// return which literal matched and the remaining segments.
fn take_literal_any<'s, 'a, 'l>(
    segments: &'s [Segment<'a>],
    literals: &[&'l str],
) -> Option<(&'l str, &'s [Segment<'a>])> {
    let ((_, value), rest) = segments.split_first()?;
    literals
        .iter()
        .find(|literal| **literal == *value)
        .map(|literal| (*literal, rest))
}

fn match_github<'a>(segments: &[Segment<'a>]) -> Option<(Segment<'a>, RefOutcome)> {
    // /{owner}/{repo}/(blob|tree|raw|edit|blame)/{ref}/{path}
    let (_owner, segments) = take_capture(segments)?;
    let (_repo, segments) = take_capture(segments)?;
    let (_verb, segments) = take_literal_any(segments, &["blob", "tree", "raw", "edit", "blame"])?;
    let (reference, _segments) = take_capture(segments)?;
    Some((reference, RefOutcome::MustBeSha))
}

fn match_gitlab<'a>(segments: &[Segment<'a>]) -> Option<(Segment<'a>, RefOutcome)> {
    // /{group(/subgroup)*}/{repo}/-/(blob|tree|raw|edit)/{ref}/{path}
    // The owner may span several subgroup segments, so the reliable
    // anchor is the `-` separator rather than a fixed segment index.
    let dash_index = segments.iter().position(|(_, value)| *value == "-")?;
    // At least an owner and a repo must precede the separator.
    if dash_index < 2 {
        return None;
    }
    let after = &segments[dash_index + 1..];
    let (_verb, after) = take_literal_any(after, &["blob", "tree", "raw", "edit"])?;
    let (reference, _after) = take_capture(after)?;
    Some((reference, RefOutcome::MustBeSha))
}

fn match_bitbucket<'a>(segments: &[Segment<'a>]) -> Option<(Segment<'a>, RefOutcome)> {
    // /{owner}/{repo}/src/{ref}/{path}
    let (_owner, segments) = take_capture(segments)?;
    let (_repo, segments) = take_capture(segments)?;
    let segments = take_literal(segments, "src")?;
    let (reference, _segments) = take_capture(segments)?;
    Some((reference, RefOutcome::MustBeSha))
}

fn match_gitea<'a>(segments: &[Segment<'a>]) -> Option<(Segment<'a>, RefOutcome)> {
    // /{owner}/{repo}/(src|raw)/(branch|commit|tag)/{ref}/{path}
    let (_owner, segments) = take_capture(segments)?;
    let (_repo, segments) = take_capture(segments)?;
    let (_root, segments) = take_literal_any(segments, &["src", "raw"])?;
    let (ref_kind, segments) = take_literal_any(segments, &["branch", "commit", "tag"])?;
    let (reference, _segments) = take_capture(segments)?;
    let outcome = match ref_kind {
        "branch" => RefOutcome::KnownMutable(MutableKind::Branch),
        "tag" => RefOutcome::KnownMutable(MutableKind::Tag),
        // "commit": the ref is claimed to be a commit, but a non-hex
        // value in the slot is still rejected, so fall through to the
        // SHA-shape check.
        _ => RefOutcome::MustBeSha,
    };
    Some((reference, outcome))
}

fn match_sourcehut<'a>(segments: &[Segment<'a>]) -> Option<(Segment<'a>, RefOutcome)> {
    // /~{user}/{repo}/(tree|blob)/{ref}/item/{path}
    let (_user, segments) = take_capture(segments)?;
    let (_repo, segments) = take_capture(segments)?;
    let (_verb, segments) = take_literal_any(segments, &["tree", "blob"])?;
    let (reference, _segments) = take_capture(segments)?;
    Some((reference, RefOutcome::MustBeSha))
}

#[cfg(test)]
mod tests;
