#[test]
fn ui() {
    // Pin the `ui/` sweep to an empty `dylint.toml` so `dylint_testing`
    // doesn't fall back to the crate's own `dylint.toml` which contains
    // non-default settings specific to this repo.
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui")
        .dylint_toml("")
        .run();
}
