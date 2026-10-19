use super::normalize;

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

/// GitLab subgroups nest beyond owner/repo.
#[test]
fn deep_subgroup_path_is_kept() {
    assert_eq!(
        normalize("git@gitlab.com:group/subgroup/repo.git").as_deref(),
        Some("https://gitlab.com/group/subgroup/repo"),
    );
}

/// An owner with no repository can't yield issue / PR links.
#[test]
fn single_segment_path_is_rejected() {
    assert_eq!(normalize("https://github.com/owner"), None);
    assert_eq!(normalize("git@github.com:owner"), None);
}

#[test]
fn missing_path_is_rejected() {
    assert_eq!(normalize("https://github.com"), None);
}

/// A port-only authority has no host, so reject it (the kept
/// `:port` must not masquerade as a host under `keep_port`).
#[test]
fn empty_host_is_rejected() {
    assert_eq!(normalize("https://:8443/owner/repo"), None);
    assert_eq!(normalize("ssh://:22/owner/repo"), None);
}

/// A pasted browser URL may carry `?tab=...` / `#anchor`;
/// neither belongs in the repo path or the generated links.
#[test]
fn query_and_fragment_are_stripped() {
    assert_eq!(
        normalize("https://github.com/owner/repo?tab=readme").as_deref(),
        Some("https://github.com/owner/repo"),
    );
    assert_eq!(
        normalize("https://github.com/owner/repo#section").as_deref(),
        Some("https://github.com/owner/repo"),
    );
    // `.git` followed by a query is still stripped to the repo.
    assert_eq!(
        normalize("https://github.com/owner/repo.git?x=1").as_deref(),
        Some("https://github.com/owner/repo"),
    );
}

/// A `?`/`#` ahead of the path is malformed; stripping it
/// leaves no `owner/repo`, so the URL is rejected rather than
/// leaking the query into the host and emitting a broken link.
#[test]
fn query_before_path_is_rejected() {
    assert_eq!(normalize("https://github.com?x=1/owner/repo"), None);
}

#[test]
fn unparseable_input_is_rejected() {
    assert_eq!(normalize("not a url"), None);
    assert_eq!(normalize(""), None);
}

/// A self-hosted forge on a non-default web port must keep it,
/// or every suggested link points at the wrong (default) port.
#[test]
fn http_port_is_preserved() {
    assert_eq!(
        normalize("https://git.example.com:8443/owner/repo").as_deref(),
        Some("https://git.example.com:8443/owner/repo"),
    );
}

/// The SSH port is not the web port, so the web base drops it.
#[test]
fn ssh_port_is_dropped() {
    assert_eq!(
        normalize("ssh://git@git.example.com:2222/owner/repo.git").as_deref(),
        Some("https://git.example.com/owner/repo"),
    );
}

/// RFC 3986 §3.1: schemes are case-insensitive. A valid
/// uppercase scheme must parse, not fall through to a panic.
#[test]
fn scheme_match_is_case_insensitive() {
    assert_eq!(
        normalize("HTTPS://github.com/owner/repo").as_deref(),
        Some("https://github.com/owner/repo"),
    );
    assert_eq!(
        normalize("SSH://git@github.com/owner/repo.git").as_deref(),
        Some("https://github.com/owner/repo"),
    );
}
