use super::config_section;
use crate::model::{ConfigDoc, ConfigField, Optionality};

#[test]
fn config_section_badges_mandatory_and_optional_fields() {
    // A mandatory field renders the `mandatory` badge; an ordinary
    // field keeps `optional`. Guards the HTML path the way
    // `render_md`'s test guards the markdown path.
    let config = ConfigDoc {
        key: "perfectionist::demo_rule".to_owned(),
        fields: vec![
            ConfigField {
                name: "style".to_owned(),
                type_label: "Style".to_owned(),
                doc_markdown: "Pick a style.".to_owned(),
                optionality: Optionality::Mandatory,
            },
            ConfigField {
                name: "extras".to_owned(),
                type_label: "[string]".to_owned(),
                doc_markdown: "Extra entries.".to_owned(),
                optionality: Optionality::Optional,
            },
        ],
        custom_types: Vec::new(),
    };
    let html = config_section(&config).into_string();
    // Tie each badge to its field so a mis-mapping (mandatory <->
    // optional) is caught, not just badge presence.
    assert!(
        html.contains(
            r#"<code class="config-key">style</code> : <code class="config-type">Style</code> <span class="badge badge-mandatory">mandatory</span>"#
        ),
        "the required field `style` should render the mandatory badge: {html}",
    );
    assert!(
        html.contains(
            r#"<code class="config-key">extras</code> : <code class="config-type">[string]</code> <span class="badge badge-optional">optional</span>"#
        ),
        "the optional field `extras` should render the optional badge: {html}",
    );
}

#[test]
fn config_key_offers_a_line_break_after_every_underscore() {
    // The `dylint.toml` table header names the same identifier the
    // index table does, in a sentence just as liable to be squeezed
    // on a narrow viewport, so it breaks on `_` boundaries too. The
    // namespace stays whole: it carries no `_`. Both quotes of the
    // `["<key>"]` header are built from quote-bearing string
    // fragments of their own, so they are pinned here as well.
    let config = ConfigDoc {
        key: "perfectionist::demo_rule".to_owned(),
        fields: vec![ConfigField {
            name: "some_field".to_owned(),
            type_label: "bool".to_owned(),
            doc_markdown: String::new(),
            optionality: Optionality::Optional,
        }],
        custom_types: Vec::new(),
    };
    let html = config_section(&config).into_string();
    assert!(
        html.contains("[&quot;perfectionist::demo_<wbr>rule&quot;]"),
        "the dylint.toml key must break on `_` boundaries, got: {html}",
    );
}
