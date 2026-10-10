use super::{ICONS, sprite, symbol};

#[test]
fn every_vendored_icon_becomes_a_symbol() {
    // The whole set, so a file that stops parsing fails here rather
    // than by rendering a page with one icon silently missing.
    for icon in ICONS {
        let symbol = symbol(icon.id, icon.source)
            .unwrap_or_else(|error| panic!("{} should rewrite: {error}", icon.id));
        assert!(
            symbol.starts_with(&format!(r#"<symbol id="{}" viewBox=""#, icon.id)),
            "{} opened with {symbol:.60}",
            icon.id,
        );
        assert!(symbol.ends_with("</symbol>"), "{} was left open", icon.id);
        assert!(
            symbol.contains("<path"),
            "{} kept no drawing: {symbol}",
            icon.id,
        );
    }
}

#[test]
fn the_rewrite_keeps_the_upstream_path_verbatim() {
    // What a reader actually sees is the path data, so it is the one
    // thing the rewrite must not touch.
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M1 2h3Z"/></svg>"#;
    assert_eq!(
        symbol("icon-demo", source).expect("should rewrite"),
        r#"<symbol id="icon-demo" viewBox="0 0 16 16"><path d="M1 2h3Z"/></symbol>"#,
    );
}

#[test]
fn the_root_keeps_nothing_that_would_override_the_stylesheet() {
    // `width` and `height` would pin the drawn size, and `fill` would
    // beat the `currentColor` the icons are themed through. An
    // upstream file carrying all three must lose all three.
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="#000000"><path d="M1 2h3Z"/></svg>"##;
    let symbol = symbol("icon-demo", source).expect("should rewrite");
    assert!(!symbol.contains("width="), "kept width: {symbol}");
    assert!(!symbol.contains("height="), "kept height: {symbol}");
    assert!(!symbol.contains("fill="), "kept fill: {symbol}");
}

#[test]
fn a_prolog_and_comments_are_skipped() {
    // Not every publisher writes the bare element the current files
    // do, and a vendored file is replaced wholesale on an upgrade.
    let source = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!-- a note from whoever drew it -->\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\"><path d=\"M1 2h3Z\"/></svg>";
    assert_eq!(
        symbol("icon-demo", source).expect("should rewrite"),
        r#"<symbol id="icon-demo" viewBox="0 0 16 16"><path d="M1 2h3Z"/></symbol>"#,
    );
}

#[test]
fn nested_elements_are_kept_whole() {
    // A `<g>` closes before the root does, so the scan has to tell the
    // two `</>`s apart rather than stop at the first.
    let source = r#"<svg viewBox="0 0 16 16"><g opacity=".5"><path d="M1 2h3Z"/></g><path d="M4 5h6Z"/></svg>"#;
    assert_eq!(
        symbol("icon-demo", source).expect("should rewrite"),
        r#"<symbol id="icon-demo" viewBox="0 0 16 16"><g opacity=".5"><path d="M1 2h3Z"/></g><path d="M4 5h6Z"/></symbol>"#,
    );
}

#[test]
fn an_attribute_value_may_hold_the_tag_delimiter() {
    // The case that makes this worth a parser: scanning for the first
    // `>` would cut the root element in half.
    let source = r#"<svg viewBox="0 0 16 16" aria-label="a > b"><path d="M1 2h3Z"/></svg>"#;
    assert_eq!(
        symbol("icon-demo", source).expect("should rewrite"),
        r#"<symbol id="icon-demo" viewBox="0 0 16 16"><path d="M1 2h3Z"/></symbol>"#,
    );
}

#[test]
fn a_root_without_a_view_box_is_refused() {
    // Without it a `<use>` cannot scale the drawing, so the icon would
    // render at whatever size the symbol happened to imply.
    let error = symbol(
        "icon-demo",
        r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#,
    )
    .expect_err("should refuse");
    assert!(error.contains("viewBox"), "unhelpful error: {error}");
}

#[test]
fn a_file_with_no_svg_is_refused() {
    let error =
        symbol("icon-demo", "<html><body>not an icon</body></html>").expect_err("should refuse");
    assert!(error.contains("no root <svg>"), "unhelpful error: {error}");
}

#[test]
fn the_sprite_holds_every_icon() {
    let sprite = sprite();
    for icon in ICONS {
        assert!(
            sprite.contains(&format!(r#"id="{}""#, icon.id)),
            "sprite is missing {}",
            icon.id,
        );
    }
    assert!(sprite.starts_with("<svg "), "sprite is not an <svg>");
    assert!(sprite.ends_with("</svg>"), "sprite was left open");
}
