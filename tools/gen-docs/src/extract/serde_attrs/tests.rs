use super::{apply_rename_all, pascal_to_snake, serde_has_default, serde_str_attr};
use syn::Attribute;

fn parse_attrs(source: &str) -> Vec<Attribute> {
    let parsed: syn::ItemStruct =
        syn::parse_str(source).expect("test input should parse as an item struct");
    parsed.attrs
}

#[test]
fn pascal_to_snake_basic() {
    assert_eq!(pascal_to_snake("Line"), "line");
    assert_eq!(pascal_to_snake("BlockComment"), "block_comment");
    assert_eq!(pascal_to_snake("XMLParser"), "xml_parser");
    assert_eq!(pascal_to_snake("HTTPServer"), "http_server");
    assert_eq!(pascal_to_snake("URL"), "url");
    assert_eq!(pascal_to_snake("already_snake"), "already_snake");
}

#[test]
fn serde_str_attr_branches() {
    // Picks the value of the requested key.
    let attrs = parse_attrs(r#"#[serde(rename_all = "snake_case")] struct S;"#);
    assert_eq!(
        serde_str_attr(&attrs, "rename_all"),
        Some("snake_case".to_owned()),
    );
    assert_eq!(serde_str_attr(&attrs, "rename"), None);

    // Ignores non-`serde` attributes.
    let attrs = parse_attrs(r#"#[derive(Debug)] #[other(rename = "x")] struct S;"#);
    assert_eq!(serde_str_attr(&attrs, "rename"), None);

    // Mixed Path / NameValue items inside `serde(...)`: the
    // path-form `default` is skipped, the name-value matches.
    let attrs = parse_attrs(r#"#[serde(default, rename = "foo")] struct S;"#);
    assert_eq!(serde_str_attr(&attrs, "rename"), Some("foo".to_owned()));

    // First match wins when the key appears more than once.
    let attrs = parse_attrs(r#"#[serde(rename = "first")] #[serde(rename = "second")] struct S;"#);
    assert_eq!(serde_str_attr(&attrs, "rename"), Some("first".to_owned()));

    // Non-string value is rejected (only `Lit::Str` is accepted).
    let attrs = parse_attrs(r#"#[serde(skip = true)] struct S;"#);
    assert_eq!(serde_str_attr(&attrs, "skip"), None);
}

#[test]
fn serde_has_default_detects_flag_and_path_forms() {
    // Bare flag.
    let attrs = parse_attrs(r#"#[serde(default, rename_all = "snake_case")] struct S;"#);
    assert!(serde_has_default(&attrs));
    // `default = "path"` form.
    let attrs = parse_attrs(r#"#[serde(default = "make")] struct S;"#);
    assert!(serde_has_default(&attrs));
    // No default directive.
    let attrs =
        parse_attrs(r#"#[serde(deny_unknown_fields, rename_all = "snake_case")] struct S;"#);
    assert!(!serde_has_default(&attrs));
    // Non-`serde` attributes are ignored.
    let attrs = parse_attrs(r#"#[other(default)] struct S;"#);
    assert!(!serde_has_default(&attrs));
}

#[test]
fn apply_rename_all_covers_serde_styles() {
    assert_eq!(
        apply_rename_all("snake_case", "BlockComment"),
        "block_comment",
    );
    assert_eq!(
        apply_rename_all("SCREAMING_SNAKE_CASE", "BlockComment"),
        "BLOCK_COMMENT",
    );
    assert_eq!(
        apply_rename_all("kebab-case", "BlockComment"),
        "block-comment",
    );
    assert_eq!(
        apply_rename_all("SCREAMING-KEBAB-CASE", "BlockComment"),
        "BLOCK-COMMENT",
    );
    assert_eq!(
        apply_rename_all("PascalCase", "BlockComment"),
        "BlockComment",
    );
    assert_eq!(
        apply_rename_all("camelCase", "BlockComment"),
        "blockComment",
    );
    assert_eq!(
        apply_rename_all("lowercase", "BlockComment"),
        "blockcomment",
    );
    assert_eq!(
        apply_rename_all("UPPERCASE", "BlockComment"),
        "BLOCKCOMMENT",
    );
    // The unknown-style fallback is observable — it prints a
    // warning and returns the name unchanged — but asserting
    // it here would spam stderr on every clean test run. The
    // behaviour is covered by manual smoke runs of `gen-docs`
    // against rule sources that intentionally use an unknown
    // style.
}
