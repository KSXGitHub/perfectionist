//! The one stylesheet the build generates rather than ships: how the
//! dark syntax sheet is scoped so it re-colours the light one's classes
//! only when the effective theme is dark.

#[test]
fn dark_highlight_css_is_scoped_to_the_dark_scheme() {
    let dark = &crate::render::markdown::HIGHLIGHT_CSS.dark;
    // The dark syntax sheet re-styles the same classes the light sheet
    // emits, so it must be scoped under a higher-specificity ancestor
    // for both the system-preference and explicit-override tiers, or it
    // would unconditionally clobber the light colours.
    assert!(dark.contains("@media (prefers-color-scheme: dark)"));
    assert!(
        dark.contains(r#"html:not([color-scheme-override="light"]) .comment"#),
        "system-preference dark highlight must scope the syntax classes",
    );
    assert!(
        dark.contains(r#"html[color-scheme-override="dark"] .comment"#),
        "explicit-Dark highlight must scope the syntax classes",
    );
}
