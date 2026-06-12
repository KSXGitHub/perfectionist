// The test inputs are deliberately branch-ref forge URLs written as
// string literals, which is exactly what this rule flags. Under
// `cargo dylint` (where the `perfectionist` tool is registered) the
// rule's own pass would fire on them; the `cfg_attr` gate keeps the
// suppression from becoming an `unknown_lints` warning under a plain
// `cargo build` / `cargo clippy`, where the tool isn't loaded.
#![cfg_attr(
    dylint_lib = "perfectionist",
    allow(
        perfectionist::unpinned_repo_ref,
        reason = "test inputs are intentionally unpinned forge URLs"
    )
)]

use super::{MutableKind, RefOutcome, RefProblem, locate_ref, ref_problem, url_host};
use crate::rules::unpinned_repo_ref::config::ForgeKind;

/// Locate the ref of `url` under `kind` and assert it sits at the
/// expected substring of `url` (so the byte offset, not just the text,
/// is verified).
fn assert_ref(url: &str, kind: ForgeKind, expected: &str, outcome: RefOutcome) {
    let location = locate_ref(url, kind).expect("expected a ref match");
    assert_eq!(location.text, expected, "ref text for {url}");
    assert_eq!(
        &url[location.offset..location.offset + location.text.len()],
        expected,
        "offset for {url}",
    );
    assert_eq!(location.outcome, outcome, "outcome for {url}");
}

#[test]
fn url_host_strips_scheme_port_and_path() {
    assert_eq!(
        url_host("https://github.com/owner/repo"),
        Some("github.com"),
    );
    assert_eq!(
        url_host("https://git.example.com:8443/owner/repo/blob/main/x"),
        Some("git.example.com"),
    );
    // Userinfo (with or without a password) is stripped before the
    // host, and a port after the host is dropped too.
    assert_eq!(
        url_host("https://user@github.com/owner/repo"),
        Some("github.com"),
    );
    assert_eq!(
        url_host("https://user:password@git.example.com:8443/g/r/-/blob/main/x"),
        Some("git.example.com"),
    );
    assert_eq!(url_host("https://github.com"), Some("github.com"));
    assert_eq!(url_host("not a url"), None);
}

#[test]
fn github_blob_branch_ref() {
    assert_ref(
        "https://github.com/owner/repo/blob/main/src/lib.rs",
        ForgeKind::Github,
        "main",
        RefOutcome::MustBeSha,
    );
}

#[test]
fn github_accepts_every_verb() {
    for verb in ["blob", "tree", "raw", "edit", "blame"] {
        let url = format!("https://github.com/owner/repo/{verb}/main/x");
        assert_ref(&url, ForgeKind::Github, "main", RefOutcome::MustBeSha);
    }
}

#[test]
fn github_commit_and_compare_urls_are_not_file_references() {
    // `/commit/<sha>` and `/compare/...` are `commit-id-length`'s
    // concern, not this rule's; they don't match the file-reference
    // shape.
    assert!(locate_ref("https://github.com/o/r/commit/abc123", ForgeKind::Github).is_none());
    assert!(locate_ref("https://github.com/o/r/compare/a...b", ForgeKind::Github).is_none());
}

#[test]
fn gitlab_uses_the_dash_separator_and_allows_subgroups() {
    assert_ref(
        "https://gitlab.com/group/repo/-/blob/main/src/lib.rs",
        ForgeKind::Gitlab,
        "main",
        RefOutcome::MustBeSha,
    );
    assert_ref(
        "https://gitlab.com/group/subgroup/repo/-/tree/develop/dir",
        ForgeKind::Gitlab,
        "develop",
        RefOutcome::MustBeSha,
    );
}

#[test]
fn gitlab_without_dash_separator_does_not_match() {
    assert!(
        locate_ref(
            "https://gitlab.com/group/repo/blob/main/x",
            ForgeKind::Gitlab
        )
        .is_none(),
    );
}

#[test]
fn bitbucket_src_ref() {
    assert_ref(
        "https://bitbucket.org/owner/repo/src/master/file.rs",
        ForgeKind::Bitbucket,
        "master",
        RefOutcome::MustBeSha,
    );
}

#[test]
fn gitea_branch_tag_commit_are_distinguished_by_path() {
    assert_ref(
        "https://codeberg.org/owner/repo/src/branch/main/src/lib.rs",
        ForgeKind::Gitea,
        "main",
        RefOutcome::KnownMutable(MutableKind::Branch),
    );
    assert_ref(
        "https://codeberg.org/owner/repo/src/tag/v1.2.3/src/lib.rs",
        ForgeKind::Gitea,
        "v1.2.3",
        RefOutcome::KnownMutable(MutableKind::Tag),
    );
    assert_ref(
        "https://codeberg.org/owner/repo/src/commit/8c1f6e2/src/lib.rs",
        ForgeKind::Gitea,
        "8c1f6e2",
        RefOutcome::MustBeSha,
    );
}

#[test]
fn sourcehut_tree_ref() {
    assert_ref(
        "https://git.sr.ht/~user/repo/tree/main/item/src/lib.rs",
        ForgeKind::Sourcehut,
        "main",
        RefOutcome::MustBeSha,
    );
}

#[test]
fn ref_problem_accepts_sha_and_rejects_short_or_non_hex() {
    // A long, pure-hex ref is accepted (no problem).
    assert_eq!(
        ref_problem("8c1f6e2a6d33", RefOutcome::MustBeSha, 4, false),
        None,
    );
    // Shorter than the recognition length: treated as a branch.
    assert_eq!(
        ref_problem("abc", RefOutcome::MustBeSha, 4, false),
        Some(RefProblem::NotSha),
    );
    // Non-hex: a branch.
    assert_eq!(
        ref_problem("main", RefOutcome::MustBeSha, 4, false),
        Some(RefProblem::NotSha),
    );
    // The recognition length is honoured: 4 hex chars pass at 4.
    assert_eq!(ref_problem("dead", RefOutcome::MustBeSha, 4, false), None);
    // ...but the same ref fails when the window is widened.
    assert_eq!(
        ref_problem("dead", RefOutcome::MustBeSha, 7, false),
        Some(RefProblem::NotSha),
    );
}

#[test]
fn ref_problem_accepts_version_patterns_when_configured() {
    for reference in [
        "1.2.3",
        "1.2.3-rc.1",
        "v1.2.3",
        "v1.2.3-suffix",
        "1.2.3-rc_1",
        "v1.2.3-alpha-2",
    ] {
        assert_eq!(
            ref_problem(reference, RefOutcome::MustBeSha, 4, true),
            None,
            "version-pattern ref {reference:?}",
        );
        assert_eq!(
            ref_problem(
                reference,
                RefOutcome::KnownMutable(MutableKind::Tag),
                4,
                true
            ),
            None,
            "gitea tag ref {reference:?}",
        );
    }

    for reference in [
        "v1.2",
        "1.2.3-",
        "release-1.2.3",
        // A suffix may only contain ASCII letters, ASCII digits, `.`,
        // `-`, and `_`; anything else is not a versioning suffix.
        "1.2.3-abc$def&ghi",
        "1.2.3-abc,def",
        "1.2.3-abc;def",
        "v1.2.3-abc/def",
    ] {
        assert_eq!(
            ref_problem(reference, RefOutcome::MustBeSha, 4, true),
            Some(RefProblem::NotSha),
            "non-version-pattern ref {reference:?}",
        );
    }
}

#[test]
fn ref_problem_rejects_version_patterns_by_default() {
    for reference in ["1.2.3", "1.2.3-rc.1", "v1.2.3", "v1.2.3-suffix"] {
        assert_eq!(
            ref_problem(reference, RefOutcome::MustBeSha, 4, false),
            Some(RefProblem::NotSha),
            "version-pattern ref {reference:?}",
        );
        assert_eq!(
            ref_problem(
                reference,
                RefOutcome::KnownMutable(MutableKind::Tag),
                4,
                false
            ),
            Some(RefProblem::Tag),
            "gitea tag ref {reference:?}",
        );
    }
}

#[test]
fn ref_problem_reports_gitea_branch_and_non_version_pattern_tag() {
    // A gitea `/branch/` URL whose ref happens to be hex is still a
    // branch — the path, not the text, decides.
    assert_eq!(
        ref_problem(
            "dead",
            RefOutcome::KnownMutable(MutableKind::Branch),
            4,
            false
        ),
        Some(RefProblem::Branch),
    );
    assert_eq!(
        ref_problem(
            "8c1f6e2",
            RefOutcome::KnownMutable(MutableKind::Tag),
            4,
            false
        ),
        Some(RefProblem::Tag),
    );
}

#[test]
fn query_and_fragment_are_stripped_before_segmenting() {
    assert_ref(
        "https://github.com/owner/repo/blob/main/src/lib.rs#L10",
        ForgeKind::Github,
        "main",
        RefOutcome::MustBeSha,
    );
}
