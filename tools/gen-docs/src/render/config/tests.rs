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
