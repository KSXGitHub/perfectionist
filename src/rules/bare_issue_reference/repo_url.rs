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
    let (scheme, rest, sep) = take_transport(input.trim())?;
    let (authority, path) = split_once_char(rest, sep)?;
    let host = take_host(authority);
    let path = take_repo_path(path)?;
    if host.is_empty() {
        return None;
    }
    Some(format!("{scheme}://{host}/{path}"))
}

/// Strip a recognised transport prefix, returning the web scheme to
/// emit, the remaining `authority<sep>path`, and which separator
/// splits them. The scp-like form has no prefix to strip, so the whole
/// string is handed back for the colon split.
fn take_transport(input: &str) -> Option<(&'static str, &str, Sep)> {
    if let Some(rest) = input.strip_prefix("https://") {
        Some(("https", rest, Sep::Slash))
    } else if let Some(rest) = input.strip_prefix("http://") {
        Some(("http", rest, Sep::Slash))
    } else if let Some(rest) = input.strip_prefix("ssh://") {
        Some(("https", rest, Sep::Slash))
    } else if is_scp_like(input) {
        Some(("https", input, Sep::Colon))
    } else {
        None
    }
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

/// Take the host out of an authority component `[user@]host[:port]`,
/// dropping the userinfo before any `@` and the port after any `:`.
fn take_host(authority: &str) -> &str {
    let after_user = match authority.rsplit_once('@') {
        Some((_, host)) => host,
        None => authority,
    };
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
    fn unparseable_input_is_rejected() {
        assert_eq!(normalize("not a url"), None);
        assert_eq!(normalize(""), None);
    }
}
