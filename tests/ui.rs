#[test]
fn ui() {
    // Pin the default-config sweep to the empty `dylint.toml` rather
    // than letting `dylint_testing` fall back to the crate's own
    // `dylint.toml`. The crate enables a non-default knob there
    // (`unicode_ellipsis_in_docs.scan_code_spans`) to dogfood it during
    // self-lint; without this pin that override would leak in and flag
    // the `ui/unicode_ellipsis_in_docs.rs` fixture's inline-code-span
    // case, which exists precisely to pin the default-skip behaviour.
    // The enabled behaviour is covered separately under `ui-toml/`.
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui")
        .dylint_toml("")
        .run();
}
