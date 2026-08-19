use super::{EMPTY_CONFIG_DOC, extract_config};
use crate::extract::shared::SharedTypes;
use crate::model::Optionality;
use std::path::Path;

#[test]
fn required_field_is_detected_syntactically() {
    // A `Config` without a container `#[serde(default)]` makes a
    // non-`Option`, non-defaulted field required — the signal for a
    // mandatory direction field. `Option` fields and defaulted
    // fields stay optional.
    let source = r#"
        const CONFIG_KEY: &str = "perfectionist::demo";

        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields, rename_all = "snake_case")]
        struct Config {
            style: Style,
            maybe: Option<Style>,
            #[serde(default)]
            with_default: Style,
        }
    "#;
    let file = syn::parse_file(source).unwrap();
    let config = extract_config(Path::new("synthetic.rs"), &file, &SharedTypes::default());
    let optionality: Vec<(&str, Optionality)> = config
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.optionality))
        .collect();
    assert_eq!(
        optionality,
        vec![
            ("style", Optionality::Mandatory),
            ("maybe", Optionality::Optional),
            ("with_default", Optionality::Optional),
        ],
    );
}

#[test]
fn container_default_makes_all_fields_optional() {
    // With a container `#[serde(default)]`, even a non-`Option`
    // field is optional (filled from the struct's default).
    let source = r#"
        const CONFIG_KEY: &str = "perfectionist::demo";

        #[derive(Default, serde::Deserialize)]
        #[serde(default, rename_all = "snake_case")]
        struct Config {
            count: usize,
        }
    "#;
    let file = syn::parse_file(source).unwrap();
    let config = extract_config(Path::new("synthetic.rs"), &file, &SharedTypes::default());
    assert_eq!(config.fields[0].optionality, Optionality::Optional);
}

#[test]
fn config_field_honours_serde_rename() {
    // Walk a synthetic rule file through `extract_config` to
    // confirm that `#[serde(rename = "...")]` on a Config
    // field surfaces as the TOML key instead of the Rust ident.
    let source = r#"
        const CONFIG_KEY: &str = "perfectionist::demo";

        #[derive(serde::Deserialize)]
        #[serde(default, rename_all = "snake_case")]
        struct Config {
            #[serde(rename = "renamed-key")]
            rust_name: bool,
            plain_name: usize,
        }
    "#;
    let file = syn::parse_file(source).unwrap();
    let config = extract_config(Path::new("synthetic.rs"), &file, &SharedTypes::default());
    let names: Vec<&str> = config.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, vec!["renamed-key", "plain_name"]);
}

#[test]
fn config_field_honours_struct_rename_all() {
    // `rename_all = "kebab-case"` on the Config struct should
    // surface every Rust field's snake-case identifier with
    // dashes. A per-field `#[serde(rename)]` still wins.
    let source = r#"
        const CONFIG_KEY: &str = "perfectionist::demo";

        #[derive(serde::Deserialize)]
        #[serde(default, rename_all = "kebab-case")]
        struct Config {
            also_flag: bool,
            some_other_key: usize,
            #[serde(rename = "explicit")]
            overridden: bool,
        }
    "#;
    let file = syn::parse_file(source).unwrap();
    let config = extract_config(Path::new("synthetic.rs"), &file, &SharedTypes::default());
    let names: Vec<&str> = config.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, vec!["also-flag", "some-other-key", "explicit"]);
}

#[test]
#[should_panic(expected = "must declare both")]
fn extract_config_rejects_a_rule_with_no_config() {
    // A rule file that declares neither `CONFIG_KEY` nor `Config`
    // is a convention violation: "no configuration" must be spelt
    // as an empty `Config {}`, never as an absent pair. The
    // extractor panics so a doc regeneration catches it instead of
    // silently dropping the configuration section.
    let source = "pub struct SomeRule;";
    let file = syn::parse_file(source).unwrap();
    let _ = extract_config(Path::new("no_config.rs"), &file, &SharedTypes::default());
}

#[test]
#[should_panic(expected = "no `Config` struct")]
fn extract_config_rejects_a_half_defined_config_key_only() {
    // Declaring `CONFIG_KEY` without a `Config` struct is the
    // usual shape of an author having dropped one half; reject it.
    let source = r#"const CONFIG_KEY: &str = "perfectionist::demo";"#;
    let file = syn::parse_file(source).unwrap();
    let _ = extract_config(Path::new("half.rs"), &file, &SharedTypes::default());
}

#[test]
#[should_panic(expected = "no `CONFIG_KEY` const")]
fn extract_config_rejects_a_half_defined_struct_only() {
    let source = "struct Config {}";
    let file = syn::parse_file(source).unwrap();
    let _ = extract_config(Path::new("half.rs"), &file, &SharedTypes::default());
}

/// A rule source whose `Config` carries `doc_lines` as its doc
/// comment and `body` as its field list. Building the doc from
/// [`EMPTY_CONFIG_DOC`] rather than restating it keeps these tests
/// from becoming one more divergent copy of the text they exist to
/// pin.
fn config_source(doc_lines: &[&str], body: &str) -> String {
    let doc: String = doc_lines
        .iter()
        .map(|line| format!("        /// {line}\n"))
        .collect();
    format!(
        "        const CONFIG_KEY: &str = \"perfectionist::demo\";\n\n{doc}        struct Config {body}\n",
    )
}

#[test]
fn empty_config_accepts_the_fixed_doc_comment() {
    let source = config_source(EMPTY_CONFIG_DOC, "{}");
    let file = syn::parse_file(&source).unwrap();
    let config = extract_config(Path::new("demo.rs"), &file, &SharedTypes::default());
    assert!(config.fields.is_empty());
}

#[test]
#[should_panic(expected = "fixed text, verbatim")]
fn empty_config_rejects_a_bespoke_doc_comment() {
    // The shape this guard exists to stop: a knob-less rule
    // explaining its empty `Config` in its own words.
    let source = config_source(
        &["Configuration is reserved for future knobs; the lint has none."],
        "{}",
    );
    let file = syn::parse_file(&source).unwrap();
    let _ = extract_config(Path::new("demo.rs"), &file, &SharedTypes::default());
}

#[test]
#[should_panic(expected = "still carries the knob-less")]
fn configured_config_rejects_the_knob_less_doc_comment() {
    // The other direction: a rule gained a knob and the doc kept
    // insisting it has none.
    let source = config_source(EMPTY_CONFIG_DOC, "{ count: usize }");
    let file = syn::parse_file(&source).unwrap();
    let _ = extract_config(Path::new("demo.rs"), &file, &SharedTypes::default());
}
