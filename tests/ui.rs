#[test]
fn ui() {
    // Pin the `ui/` sweep to an empty `dylint.toml` so the harness
    // doesn't fall back to the crate's own `dylint.toml` which contains
    // non-default settings specific to this repo.
    _utils::ConfiguredUiTest::builder()
        .library_name(env!("CARGO_PKG_NAME"))
        .manifest_dir(env!("CARGO_MANIFEST_DIR"))
        .src_base("ui")
        .dylint_toml("")
        .run();
}
