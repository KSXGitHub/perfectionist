//! Configured UI test for `numeric_escape_for_visible_character`. The
//! default-config sweep lives in
//! `ui/numeric_escape_for_visible_character.rs` and is picked up by
//! `tests/ui.rs`; that fixture carries both ASCII and non-ASCII
//! escapes and expects a diagnostic on each, which is what pins the
//! default. The test below narrows the rule through its one knob.

use text_block_macros::text_block_fnl;

fn run(src_base: &str, contents: &str) {
    _utils::ConfiguredUiTest::builder()
        .library_name(env!("CARGO_PKG_NAME"))
        .manifest_dir(env!("CARGO_MANIFEST_DIR"))
        .src_base(src_base)
        .dylint_toml(contents)
        .run();
}

/// `flag_non_ascii = false` is what a project running
/// `clippy::non_ascii_literal` sets, so that this rule never
/// suggests spelling a non-ASCII character out, which is the thing
/// that policy forbids. The ASCII escapes in the fixture still fire;
/// the non-ASCII ones are left alone.
#[test]
fn flag_non_ascii_disabled_keeps_only_the_ascii_half() {
    run(
        "ui-toml/numeric_escape_for_visible_character/ascii_only",
        text_block_fnl! {
            r#"["perfectionist::numeric_escape_for_visible_character"]"#
            "flag_non_ascii = false"
        },
    );
}
