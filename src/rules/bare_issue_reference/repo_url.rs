//! Parse a repository URL — in any form a user is likely to clone or
//! paste — into the canonical web base `https://{host}/{owner}/{repo}`
//! that `bare_issue_reference` builds issue / PR links from.
//!
//! Three input shapes are accepted:
//!
//! * an HTTP(S) URL — `https://host/owner/repo[.git]`
//! * an `ssh://` URL — `ssh://[user@]host[:port]/owner/repo[.git]`
//! * the scp-like shorthand — `[user@]host:owner/repo[.git]`
//!
//! The `git@` userinfo, any `:port`, an optional `.git` suffix, and
//! surrounding slashes are all dropped. An HTTP(S) input keeps its own
//! scheme; the SSH forms (which carry no web scheme) map to `https`,
//! since only the host + owner/repo matter for a *web* link.
//!
//! Written as small `take_*` parser combinators per
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`; no regex.

/// How the authority is separated from the path: `/` for the URL
/// forms, `:` for the scp-like shorthand.
#[derive(Clone, Copy)]
enum Sep {
    Slash,
    Colon,
}

/// Parse `input` into the canonical web base `https://host/owner/repo`
/// (the scheme is preserved for an HTTP(S) input). Returns `None` when
/// no host and `owner/repo` path can be extracted.
pub(super) fn normalize(input: &str) -> Option<String> {
    let transport = take_transport(input.trim())?;
    let (authority, path) = split_once_char(transport.rest, transport.sep)?;
    let host = take_host(authority, transport.keep_port);
    let path = take_repo_path(path)?;
    // Reject a missing host. `starts_with(':')` catches the
    // port-only authority (`https://:8443/o/r`) that survives with
    // `keep_port` because the kept `:8443` is non-empty.
    if host.is_empty() || host.starts_with(':') {
        return None;
    }
    Some(format!("{}://{host}/{path}", transport.scheme))
}

/// A recognised transport prefix stripped from the input.
struct Transport<'a> {
    /// Web scheme to emit (`http`/`https`).
    scheme: &'static str,
    /// The remaining `authority<sep>path`.
    rest: &'a str,
    /// Separator between authority and path.
    sep: Sep,
    /// Whether the authority's `:port` is a *web* port to keep. True
    /// only for `http(s)://` inputs; for `ssh://` the port is the SSH
    /// port (not a web port), and the scp-like form has no port.
    keep_port: bool,
}

/// Strip a recognised transport prefix. The scheme is matched
/// case-insensitively (RFC 3986 §3.1). The scp-like form has no prefix
/// to strip, so the whole string is handed back for the colon split.
fn take_transport(input: &str) -> Option<Transport<'_>> {
    if let Some(rest) = strip_prefix_ci(input, "https://") {
        Some(Transport {
            scheme: "https",
            rest,
            sep: Sep::Slash,
            keep_port: true,
        })
    } else if let Some(rest) = strip_prefix_ci(input, "http://") {
        Some(Transport {
            scheme: "http",
            rest,
            sep: Sep::Slash,
            keep_port: true,
        })
    } else if let Some(rest) = strip_prefix_ci(input, "ssh://") {
        Some(Transport {
            scheme: "https",
            rest,
            sep: Sep::Slash,
            keep_port: false,
        })
    } else if is_scp_like(input) {
        Some(Transport {
            scheme: "https",
            rest: input,
            sep: Sep::Colon,
            keep_port: false,
        })
    } else {
        None
    }
}

/// `str::strip_prefix`, but matching `prefix` case-insensitively.
fn strip_prefix_ci<'a>(input: &'a str, prefix: &str) -> Option<&'a str> {
    let head = input.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &input[prefix.len()..])
}

/// Whether `input` is the scp-like shorthand `[user@]host:path`: it
/// carries no `://` scheme and its first `:` comes before any `/`
/// (git's own rule for recognising this form, which keeps a path like
/// `a/b:c` from being misread as a host/path split).
fn is_scp_like(input: &str) -> bool {
    if input.contains("://") {
        return false;
    }
    match (input.find(':'), input.find('/')) {
        (Some(colon), Some(slash)) => colon < slash,
        (Some(_), None) => true,
        _ => false,
    }
}

/// Split `input` once on the separator character, returning the part
/// before and after it. `None` when the separator is absent (a URL
/// with no path).
fn split_once_char(input: &str, sep: Sep) -> Option<(&str, &str)> {
    let delim = match sep {
        Sep::Slash => '/',
        Sep::Colon => ':',
    };
    input.split_once(delim)
}

/// Take the host (with its port when `keep_port`) out of an authority
/// component `[user@]host[:port]`, always dropping the userinfo before
/// any `@`. For an `http(s)://` input the port is the web port and is
/// kept; for `ssh://` it is the SSH port and is dropped.
fn take_host(authority: &str, keep_port: bool) -> &str {
    let after_user = match authority.rsplit_once('@') {
        Some((_, host)) => host,
        None => authority,
    };
    if keep_port {
        return after_user;
    }
    match after_user.split_once(':') {
        Some((host, _port)) => host,
        None => after_user,
    }
}

/// Take the `owner/repo` path: drop surrounding slashes and a single
/// trailing `.git`. Requires at least two segments (an `owner/repo`
/// pair, or a deeper GitLab subgroup path), so a lone `owner` with no
/// repository yields `None`.
fn take_repo_path(path: &str) -> Option<&str> {
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let path = path.trim_end_matches('/');
    path.contains('/').then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_url_passes_through() {
        assert_eq!(
            normalize("https://github.com/owner/repo").as_deref(),
            Some("https://github.com/owner/repo"),
        );
    }

    #[test]
    fn http_scheme_is_preserved() {
        assert_eq!(
            normalize("http://git.example.com/owner/repo").as_deref(),
            Some("http://git.example.com/owner/repo"),
        );
    }

    #[test]
    fn dot_git_suffix_is_dropped() {
        assert_eq!(
            normalize("https://github.com/owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo"),
        );
    }

    #[test]
    fn trailing_slash_is_dropped() {
        assert_eq!(
            normalize("https://github.com/owner/repo/").as_deref(),
            Some("https://github.com/owner/repo"),
        );
    }

    #[test]
    fn scp_like_maps_to_https() {
        assert_eq!(
            normalize("git@github.com:owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo"),
        );
    }

    #[test]
    fn scp_like_without_user_or_dot_git() {
        assert_eq!(
            normalize("github.com:owner/repo").as_deref(),
            Some("https://github.com/owner/repo"),
        );
    }

    #[test]
    fn ssh_url_drops_userinfo_and_port() {
        assert_eq!(
            normalize("ssh://git@github.com:22/owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo"),
        );
    }

    #[test]
    fn deep_subgroup_path_is_kept() {
        // GitLab subgroups nest beyond owner/repo.
        assert_eq!(
            normalize("git@gitlab.com:group/subgroup/repo.git").as_deref(),
            Some("https://gitlab.com/group/subgroup/repo"),
        );
    }

    #[test]
    fn single_segment_path_is_rejected() {
        // An owner with no repository can't yield issue / PR links.
        assert_eq!(normalize("https://github.com/owner"), None);
        assert_eq!(normalize("git@github.com:owner"), None);
    }

    #[test]
    fn missing_path_is_rejected() {
        assert_eq!(normalize("https://github.com"), None);
    }

    #[test]
    fn empty_host_is_rejected() {
        // A port-only authority has no host — reject it (the kept
        // `:port` must not masquerade as a host under `keep_port`).
        assert_eq!(normalize("https://:8443/owner/repo"), None);
        assert_eq!(normalize("ssh://:22/owner/repo"), None);
    }

    #[test]
    fn unparseable_input_is_rejected() {
        assert_eq!(normalize("not a url"), None);
        assert_eq!(normalize(""), None);
    }

    #[test]
    fn http_port_is_preserved() {
        // A self-hosted forge on a non-default web port must keep it,
        // or every suggested link points at the wrong (default) port.
        assert_eq!(
            normalize("https://git.example.com:8443/owner/repo").as_deref(),
            Some("https://git.example.com:8443/owner/repo"),
        );
    }

    #[test]
    fn ssh_port_is_dropped() {
        // The SSH port is not the web port, so the web base drops it.
        assert_eq!(
            normalize("ssh://git@git.example.com:2222/owner/repo.git").as_deref(),
            Some("https://git.example.com/owner/repo"),
        );
    }

    #[test]
    fn scheme_match_is_case_insensitive() {
        // RFC 3986 §3.1: schemes are case-insensitive. A valid
        // uppercase scheme must parse, not fall through to a panic.
        assert_eq!(
            normalize("HTTPS://github.com/owner/repo").as_deref(),
            Some("https://github.com/owner/repo"),
        );
        assert_eq!(
            normalize("SSH://git@github.com/owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo"),
        );
    }
}
