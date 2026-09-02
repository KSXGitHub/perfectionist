use super::{CACHE_DIR_ENV, DOWNLOADS, cache_dir, ensure_cached, install_into};
use std::path::{Path, PathBuf};

/// A scratch directory under the shared scratch root, removed on drop,
/// so the filesystem-touching tests don't litter or collide.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        TempDir(_utils::scratch::dir(&format!("gen-docs-fonts-{tag}")))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn downloads_list_ships_the_font_and_its_license() {
    let names: Vec<&str> = DOWNLOADS.iter().map(|&(name, _)| name).collect();
    assert_eq!(names, ["cantarell.otf", "Cantarell-OFL.txt"]);
    for &(_, url) in DOWNLOADS {
        assert!(url.starts_with("https://"), "each URL must be https: {url}");
    }
}

#[test]
fn cache_dir_defaults_under_root_but_honours_env() {
    // SAFETY: single-threaded mutation of one process-wide var, restored
    // before returning so sibling tests are unaffected.
    let previous = std::env::var_os(CACHE_DIR_ENV);
    unsafe { std::env::remove_var(CACHE_DIR_ENV) };
    assert_eq!(
        cache_dir(Path::new("/repo")),
        Path::new("/repo/.cache/fonts"),
    );
    unsafe { std::env::set_var(CACHE_DIR_ENV, "/elsewhere/fonts") };
    assert_eq!(cache_dir(Path::new("/repo")), Path::new("/elsewhere/fonts"));
    match previous {
        Some(value) => unsafe { std::env::set_var(CACHE_DIR_ENV, value) },
        None => unsafe { std::env::remove_var(CACHE_DIR_ENV) },
    }
}

#[test]
fn ensure_cached_reuses_an_existing_file_without_downloading() {
    // A cache that already holds the file must be left alone and trigger
    // no download — the offline path. We prove "no download" by pointing
    // the URL at an unroutable address: if ensure_cached tried to fetch,
    // it would fail instead of returning Ok.
    let cache = TempDir::new("reuse");
    let downloads = [("demo.woff2", "https://127.0.0.1:1/never-fetched.woff2")];
    std::fs::write(cache.path().join("demo.woff2"), b"already here").unwrap();
    ensure_cached(cache.path(), &downloads)
        .expect("an existing cached file should be reused offline");
    assert_eq!(
        std::fs::read(cache.path().join("demo.woff2")).unwrap(),
        b"already here",
    );
}

#[test]
fn install_into_hard_links_when_possible() {
    // Cache and output share a filesystem (same temp root), so the
    // install must hard-link rather than copy: the two paths end up as
    // one inode, no bytes duplicated.
    let root = TempDir::new("install");
    let cache = root.path().join("cache");
    let out = root.path().join("out");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let downloads = [("demo.woff2", "")];
    std::fs::write(cache.join("demo.woff2"), b"linked bytes").unwrap();

    install_into(&out, &cache, &downloads).expect("install should succeed");

    let dest = out.join("demo.woff2");
    assert_eq!(std::fs::read(&dest).unwrap(), b"linked bytes");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let cache_ino = std::fs::metadata(cache.join("demo.woff2")).unwrap().ino();
        let dest_ino = std::fs::metadata(&dest).unwrap().ino();
        assert_eq!(
            cache_ino, dest_ino,
            "install_into must hard-link within one filesystem, not copy",
        );
    }
}

#[test]
fn install_into_replaces_a_stale_destination() {
    // A leftover file from a previous run must not make the hard-link
    // fail (it errors if the destination exists) — install clears it
    // first, so the output always reflects the current cache.
    let root = TempDir::new("stale");
    let cache = root.path().join("cache");
    let out = root.path().join("out");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let downloads = [("demo.woff2", "")];
    std::fs::write(cache.join("demo.woff2"), b"fresh").unwrap();
    std::fs::write(out.join("demo.woff2"), b"stale").unwrap();

    install_into(&out, &cache, &downloads).expect("install should succeed");

    assert_eq!(std::fs::read(out.join("demo.woff2")).unwrap(), b"fresh");
}
