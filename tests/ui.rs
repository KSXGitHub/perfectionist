#[test]
fn ui() {
    // Pin the `ui/` sweep to an empty `dylint.toml` so the harness
    // doesn't fall back to the crate's own `dylint.toml` which contains
    // non-default settings specific to this repo.
    _utils::configured_ui_test(env!("CARGO_PKG_NAME"), env!("CARGO_MANIFEST_DIR"), "ui", "").run();
}
