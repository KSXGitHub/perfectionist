use super::render_invocation;

#[test]
fn render_without_reason() {
    assert_eq!(
        render_invocation("expect", &["clippy::foo".to_owned()], None),
        "expect(clippy::foo)",
    );
}

#[test]
fn render_with_reason() {
    assert_eq!(
        render_invocation(
            "allow",
            &["dead_code".to_owned()],
            Some(r#"reason = "scaffolding""#),
        ),
        r#"allow(dead_code, reason = "scaffolding")"#,
    );
}

#[test]
fn render_multiple_names() {
    assert_eq!(
        render_invocation(
            "expect",
            &["clippy::foo".to_owned(), "rustdoc::bar".to_owned()],
            Some(r#"reason = "x""#),
        ),
        r#"expect(clippy::foo, rustdoc::bar, reason = "x")"#,
    );
}
