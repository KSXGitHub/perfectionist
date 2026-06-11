//! Source-text scan shared by the comment surfaces and the
//! string-literal surface. [`scan_text`] walks a slice of text,
//! reusing the `bare_url` URL discovery primitives
//! ([`take_url`] plus the markdown
//! code-region mask), and hands each unpinned forge ref it finds to a
//! sink. The two callers in `unpinned_repo_ref.rs` differ only in how
//! they turn an in-text offset back into a source [`rustc_span::Span`].

use crate::markdown::{SkipRange, position_in_skip, utf8_char_len};
use crate::rules::unpinned_repo_ref::UnpinnedRepoRef;
use crate::rules::unpinned_repo_ref::parse::{RefProblem, locate_ref, ref_problem, url_host};
use crate::url_scan::{DEFAULT_FORWARD_SCHEMES, take_url};

/// Walk `text`, calling `sink(offset, length, problem, ref_text)` for
/// each unpinned forge ref. `offset` / `length` locate the ref
/// substring within `text`; `ref_text` is that substring.
///
/// `skips` are byte ranges (markdown code spans / blocks) where a URL
/// is example text, not a live link, and must be ignored — empty for
/// surfaces that aren't markdown (plain comments, string literals).
pub(super) fn scan_text(
    text: &str,
    skips: &[SkipRange],
    rule: &UnpinnedRepoRef,
    mut sink: impl FnMut(usize, usize, RefProblem, &str),
) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if !byte.is_ascii_alphabetic() {
            index += utf8_char_len(bytes, index);
            continue;
        }
        // Word-boundary guard: a scheme starting mid-identifier
        // (`xhttps://`) is not a URL. Unlike `bare_url`, no guard
        // against a preceding `<` / `[` is needed — a ref inside an
        // autolink or labelled link is still subject to pinning, so
        // wrapped URLs must be scanned too.
        if index > 0 {
            let previous = bytes[index - 1];
            if previous.is_ascii_alphanumeric() || previous == b'_' {
                index += 1;
                continue;
            }
        }
        let Some(url_match) = take_url(&text[index..], DEFAULT_FORWARD_SCHEMES) else {
            index += 1;
            continue;
        };
        if position_in_skip(skips, index) {
            index += url_match.consumed;
            continue;
        }
        if let Some((offset, length, problem, reference)) = classify_url(url_match.url, rule) {
            sink(index + offset, length, problem, reference);
        }
        index += url_match.consumed;
    }
}

/// Resolve `url`'s host to a forge kind, locate its ref, and classify
/// it. `None` when the host isn't a configured forge, the URL isn't a
/// file reference, or the ref is an acceptable SHA. The returned
/// offset is relative to the start of `url`.
fn classify_url<'a>(
    url: &'a str,
    rule: &UnpinnedRepoRef,
) -> Option<(usize, usize, RefProblem, &'a str)> {
    let host = url_host(url)?;
    let kind = rule.forge_kind_for_host(host)?;
    let location = locate_ref(url, kind)?;
    let problem = ref_problem(location.text, location.outcome, rule.sha_recognition_length)?;
    Some((location.offset, location.text.len(), problem, location.text))
}
