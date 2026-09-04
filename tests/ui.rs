#[test]
fn ui() {
    // Pin the `ui/` sweep to an empty `dylint.toml` so `dylint_testing`
    // doesn't fall back to the crate's own `dylint.toml` which contains
    // non-default settings specific to this repo.
    let fixtures = _utils::copy_fixtures_with_directive(env!("CARGO_MANIFEST_DIR"), "ui");
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml("")
        .run();
}
