//! Configuration surface for `unpinned_repo_ref`: which surfaces to
//! scan, how short a hex run still counts as a SHA, whether
//! version-shaped refs are accepted, and the hostname-to-forge-kind
//! table that decides how each URL's path is shaped.
//!
//! The hostname matching is glob-aware ([`glob_match`]) so a single
//! entry can cover a family of self-hosted subdomains
//! (`gitlab.*.example.com`). The matcher is a hand-rolled `*`
//! wildcard rather than a `regex` / `glob` dependency, in keeping with
//! the crate's "no parser library in a Dylint pass" stance.

pub(super) const CONFIG_KEY: &str = "perfectionist::unpinned_repo_ref";

/// Which text surfaces the rule scans. Each value names one of the
/// three places a forge URL is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Target {
    /// Doc comments (`///`, `//!`, `/** */`, `/*! */`).
    Doc,
    /// Regular comments (`//`, `/* */`).
    Comment,
    /// String literals (`"..."`, `r"..."`).
    StringLiteral,
}

/// The forge a hostname maps to. The variant fixes the URL shape the
/// per-kind matcher in [`super::parse`] applies; it does not vary per
/// host, so a self-hosted instance reuses an existing variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ForgeKind {
    /// `/{owner}/{repo}/(blob|tree|raw|edit|blame)/{ref}/{path}`.
    /// Also covers gitee, which mirrors GitHub's URL shape.
    Github,
    /// `/{owner}/{repo}/-/(blob|tree|raw|edit)/{ref}/{path}`. The
    /// `/-/` separator lets the owner span several subgroup segments.
    Gitlab,
    /// `/{owner}/{repo}/src/{ref}/{path}`.
    Bitbucket,
    /// `/{owner}/{repo}/(src|raw)/(branch|commit|tag)/{ref}/{path}`.
    /// The path names the ref kind, so a `/branch/` or `/tag/` URL is
    /// known-mutable without inspecting the ref text.
    Gitea,
    /// `/~{user}/{repo}/(tree|blob)/{ref}/item/{path}`.
    Sourcehut,
}

/// One row of the `hosts` table: a hostname (or glob) and the forge
/// kind whose URL shape applies to it.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct HostEntry {
    /// Hostname to match, compared case-insensitively. A `*` wildcard
    /// matches any run of characters, so `gitlab.*.example.com`
    /// covers every subdomain in one entry.
    pub(super) hostname: String,
    /// Forge kind whose URL shape applies to this hostname: one of
    /// `github` (also covers gitee), `gitlab`, `bitbucket`, `gitea`
    /// (also covers Codeberg / Forgejo), or `sourcehut`.
    pub(super) kind: ForgeKind,
}

/// Surfaces scanned when `targets` is omitted: every surface.
pub(super) const DEFAULT_TARGETS: &[Target] =
    &[Target::Doc, Target::Comment, Target::StringLiteral];

/// Minimum hex length recognised as a SHA when `sha_recognition_length`
/// is omitted — Git's own minimum abbreviated-SHA length.
pub(super) const DEFAULT_SHA_RECOGNITION_LENGTH: usize = 4;

/// The forge hosts scanned out of the box, each paired with the kind
/// that fixes its URL shape.
pub(super) const DEFAULT_HOSTS: &[(&str, ForgeKind)] = &[
    ("github.com", ForgeKind::Github),
    ("gitee.com", ForgeKind::Github),
    ("gitlab.com", ForgeKind::Gitlab),
    ("bitbucket.org", ForgeKind::Bitbucket),
    ("codeberg.org", ForgeKind::Gitea),
    ("git.sr.ht", ForgeKind::Sourcehut),
];

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Surfaces to scan, any subset of `doc`, `comment`, and
    /// `string_literal`. Defaults to
    /// `["doc", "comment", "string_literal"]`.
    pub(super) targets: Vec<Target>,
    /// Minimum hex length for a ref to be recognised as a commit SHA.
    /// A pure-hex ref shorter than this is treated as a branch and
    /// rejected. Defaults to `4` (Git's own minimum SHA length),
    /// which trades a small false-negative window (branch names like
    /// `dead`, `face`, `beef`) for fewer false positives on
    /// branch names that merely look hex-ish. Set to `1` to treat any
    /// pure-hex ref as a SHA.
    pub(super) sha_recognition_length: usize,
    /// Whether refs shaped like version patterns (`1.2.3`, `v1.2.3`, or
    /// either form with a non-empty `-suffix`) are accepted without a
    /// commit SHA. Defaults to `false`; tags can move, and a
    /// version-shaped branch name is valid Git, so projects must opt in
    /// to this convenience explicitly.
    pub(super) allow_version_patterns: bool,
    /// Hostnames to scan, each mapped to the forge kind that fixes its
    /// URL shape. Defaults to the common public forges:
    /// `github.com` / `gitee.com` (github-shape), `gitlab.com`,
    /// `bitbucket.org`, `codeberg.org` (gitea-shape), and `git.sr.ht`.
    /// Register a self-hosted instance by adding an entry with the
    /// matching `kind`. Supplying this field replaces the built-in
    /// list rather than extending it.
    pub(super) hosts: Vec<HostEntry>,
    /// Hostnames to skip, as `*`-glob patterns compared
    /// case-insensitively against the URL host. A host matching any
    /// pattern here is ignored even when it also appears in `hosts`.
    /// Defaults to `[]`.
    pub(super) skip_hosts: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            targets: DEFAULT_TARGETS.to_vec(),
            sha_recognition_length: DEFAULT_SHA_RECOGNITION_LENGTH,
            allow_version_patterns: false,
            hosts: DEFAULT_HOSTS
                .iter()
                .map(|(hostname, kind)| HostEntry {
                    hostname: (*hostname).to_owned(),
                    kind: *kind,
                })
                .collect(),
            skip_hosts: Vec::new(),
        }
    }
}

/// Match `text` against a `*`-glob `pattern`, case-insensitively over
/// ASCII. `*` matches any run of characters (including the empty run);
/// every other character matches itself. There is no `?` wildcard —
/// hostnames don't need single-character matching, and leaving it out
/// keeps the matcher a plain two-pointer scan with star backtracking.
pub(super) fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    // Two-pointer scan. `star` remembers the last `*` in the pattern
    // and `resume` the text position to retry from when a later
    // literal mismatch forces the `*` to swallow one more byte.
    let mut pattern_index = 0;
    let mut text_index = 0;
    let mut star: Option<usize> = None;
    let mut resume = 0;
    while text_index < text.len() {
        if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star = Some(pattern_index);
            resume = text_index;
            pattern_index += 1;
        } else if pattern_index < pattern.len()
            && pattern[pattern_index].eq_ignore_ascii_case(&text[text_index])
        {
            pattern_index += 1;
            text_index += 1;
        } else if let Some(star_pos) = star {
            // Mismatch under an open `*`: let it eat one more byte.
            pattern_index = star_pos + 1;
            resume += 1;
            text_index = resume;
        } else {
            return false;
        }
    }
    // Trailing `*`s in the pattern match the empty run.
    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

#[cfg(test)]
mod tests;
