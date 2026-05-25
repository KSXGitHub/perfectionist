//! Cross-rule UI test for `unicode_ellipsis_in_comments`. The
//! default-config sweep lives in `ui/unicode_ellipsis_in_comments.rs`
//! and is picked up by `tests/ui.rs`; that fixture is deliberately
//! limited to `//` and `/* */` comments so it exercises only this
//! rule. The test below points at a fixture under
//! `ui-toml/unicode_ellipsis_in_comments/` and disables the sibling
//! `unicode_ellipsis_in_docs` rule via the `[perfectionist]` global
//! table, so the doc-comment lines in that fixture isolate the claim
//! that the comment rule does not intrude into doc comments.
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared `Mutex`
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::sync::Mutex;

use text_block_macros::text_block_fnl;

static SERIAL: Mutex<()> = Mutex::new(());

fn run(src_base: &str, contents: &str) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(contents)
        .run();
}

/// With the sibling `unicode_ellipsis_in_docs` rule disabled, the
/// comment rule flags only the `//` and `/* */` comments and leaves
/// the `///` / `//!` doc comments untouched — pinning the claim that
/// the comment rule does not intrude into doc comments.
#[test]
fn comment_rule_does_not_intrude_into_doc_comments() {
    run(
        "ui-toml/unicode_ellipsis_in_comments/ignores_doc_comments",
        text_block_fnl! {
            "[perfectionist]"
            r#"disable = ["unicode_ellipsis_in_docs"]"#
        },
    );
}
