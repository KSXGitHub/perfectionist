use super::source_link_prefix_for;
use std::path::Path;

#[test]
fn source_link_prefix_for_typical_one_level() {
    let prefix = source_link_prefix_for(Path::new("/repo/rules"), Path::new("/repo"))
        .expect("typical layout should resolve");
    assert_eq!(prefix, "../");
}

/// A layout no repository here uses, pinned so the arithmetic is not
/// fitted to the one depth that does.
#[test]
fn source_link_prefix_for_deeper_nesting() {
    let prefix = source_link_prefix_for(Path::new("/repo/docs/lints/rules"), Path::new("/repo"))
        .expect("deeper layout should resolve");
    assert_eq!(prefix, "../../../");
}

/// Catalogue `README.md` would clobber the project's own.
#[test]
fn source_link_prefix_for_root_equals_rules_dir_is_rejected() {
    let error = source_link_prefix_for(Path::new("/repo"), Path::new("/repo"))
        .expect_err("rules_dir == root should be rejected");
    assert!(error.contains("same directory"), "got: {error}");
}

#[test]
fn source_link_prefix_for_outside_root_is_rejected() {
    let error = source_link_prefix_for(Path::new("/tmp/elsewhere"), Path::new("/repo"))
        .expect_err("path outside --root should be rejected");
    assert!(error.contains("not inside"), "got: {error}");
}

/// Regression test: `std::path::absolute` doesn't collapse
/// `..`, so a naive depth count would treat `rules/../rules`
/// as depth 3 and produce broken `Source:` links. Reject
/// the input instead.
#[test]
fn source_link_prefix_for_parent_dir_segment_is_rejected() {
    let error = source_link_prefix_for(Path::new("/repo/rules/../rules"), Path::new("/repo"))
        .expect_err("`..` in rules_dir should be rejected");
    assert!(error.contains("`..`"), "got: {error}");
}

/// `./rules/./` is semantically `rules/`; depth 1, not 3.
#[test]
fn source_link_prefix_for_cur_dir_segments_are_ignored() {
    let prefix = source_link_prefix_for(Path::new("/repo/./rules/."), Path::new("/repo"))
        .expect("`.` segments should be transparent");
    assert_eq!(prefix, "../");
}

/// The lexical containment check would accept a symlinked `rules_dir`,
/// the symlink's own path being under `root`. The helper refuses it
/// before an orphan-delete can follow the link out of the tree.
#[test]
fn source_link_prefix_for_rejects_symlink_rules_dir() {
    let base =
        std::env::temp_dir().join(format!("perfectionist-symlink-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let real = base.join("real");
    std::fs::create_dir_all(&real).unwrap();
    let link = base.join("rules");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real, &link).unwrap();
    #[cfg(not(unix))]
    {
        // Windows symlinks need elevated privileges; skip.
        return;
    }
    let error =
        source_link_prefix_for(&link, &base).expect_err("symlinked rules_dir should be rejected");
    assert!(error.contains("symbolic link"), "got: {error}");
}

/// Regression: `std::path::absolute` *does* normalise `.`
/// segments on every platform std supports, so a `--root`
/// passed as `/repo/.` resolves to `/repo` and strip_prefix
/// works as expected. Pin the behaviour with a test in case
/// the std contract ever loosens.
#[test]
fn source_link_prefix_for_trailing_dot_on_root() {
    let prefix = source_link_prefix_for(Path::new("/repo/rules"), Path::new("/repo/."))
        .expect("trailing `.` on --root should normalise");
    assert_eq!(prefix, "../");
}
