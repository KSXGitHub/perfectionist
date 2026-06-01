use super::*;

#[test]
fn source_link_prefix_for_typical_one_level() {
    // The canonical layout: `rules_dir` directly inside
    // `root`. Every rule .md climbs one directory.
    let prefix = source_link_prefix_for(Path::new("/repo/rules"), Path::new("/repo"))
        .expect("typical layout should resolve");
    assert_eq!(prefix, "../");
}

#[test]
fn source_link_prefix_for_deeper_nesting() {
    // Hypothetical multi-level layout. Three components →
    // three `../` segments.
    let prefix = source_link_prefix_for(Path::new("/repo/docs/lints/rules"), Path::new("/repo"))
        .expect("deeper layout should resolve");
    assert_eq!(prefix, "../../../");
}

#[test]
fn source_link_prefix_for_root_equals_rules_dir_is_rejected() {
    // Catalogue `README.md` would clobber the project's own.
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

#[test]
fn source_link_prefix_for_parent_dir_segment_is_rejected() {
    // Regression test: `std::path::absolute` doesn't collapse
    // `..`, so a naive depth count would treat `rules/../rules`
    // as depth 3 and produce broken `Source:` links. Reject
    // the input instead.
    let error = source_link_prefix_for(Path::new("/repo/rules/../rules"), Path::new("/repo"))
        .expect_err("`..` in rules_dir should be rejected");
    assert!(error.contains("`..`"), "got: {error}");
}

#[test]
fn source_link_prefix_for_cur_dir_segments_are_ignored() {
    // `./rules/./` is semantically `rules/`; depth 1, not 3.
    let prefix = source_link_prefix_for(Path::new("/repo/./rules/."), Path::new("/repo"))
        .expect("`.` segments should be transparent");
    assert_eq!(prefix, "../");
}

#[test]
fn source_link_prefix_for_rejects_symlink_rules_dir() {
    // Create a real directory and a symlink pointing at it,
    // then ask the helper to resolve the symlink's path. The
    // lexical containment check would happily accept this
    // (the symlink path is under `root`), but the helper
    // refuses before any orphan-delete can follow the link.
    // Skip on platforms where symlink creation isn't
    // supported in the test sandbox.
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

#[test]
fn source_link_prefix_for_trailing_dot_on_root() {
    // Regression: `std::path::absolute` *does* normalise `.`
    // segments on every platform std supports, so a `--root`
    // passed as `/repo/.` resolves to `/repo` and strip_prefix
    // works as expected. Pin the behaviour with a test in case
    // the std contract ever loosens.
    let prefix = source_link_prefix_for(Path::new("/repo/rules"), Path::new("/repo/."))
        .expect("trailing `.` on --root should normalise");
    assert_eq!(prefix, "../");
}
