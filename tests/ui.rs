#[test]
fn ui() {
    // Pin the `ui/` sweep to an empty `dylint.toml` so `dylint_testing`
    // doesn't fall back to the crate's own `dylint.toml`, whose
    // non-default knobs (enabled to dogfood them during self-lint) would
    // otherwise leak in and skew these default-config fixtures.
    // Non-default configs are exercised under `ui-toml/`.
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui")
        .dylint_toml("")
        .run();
}
