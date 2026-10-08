//! A rule's own entry in the catalogue: the fragment its name becomes,
//! the line-break opportunities that name carries wherever it is
//! printed, and the Configuration block beneath it.

use super::test_fixtures::{fake_context, fake_rule, sidebar_list};
use super::{anchor_for, render_page};
use crate::model::{ConfigDoc, ConfigField, Optionality};

#[test]
fn anchor_for_builds_a_kebab_cased_rule_route() {
    // The `/rule/` prefix plus a kebab-cased name: a single
    // word is unchanged, a multi-word name has every `_`
    // turned into `-`. The namespace prefix is dropped.
    assert_eq!(anchor_for("perfectionist::alpha"), "/rule/alpha");
    assert_eq!(
        anchor_for("perfectionist::beta_gamma_delta"),
        "/rule/beta-gamma-delta",
    );
}

#[test]
fn lint_names_offer_a_line_break_after_every_underscore() {
    // Nothing inside a lint identifier is a line-break opportunity,
    // so a name too long for its column is cut mid-segment by the
    // `overflow-wrap: anywhere` fallback. Every place the page
    // prints a lint name marks the segment boundaries with `<wbr>`
    // so the break lands after a `_` instead.
    let name = "redundant_derive_more_forward_template";
    let html = render_page(&[fake_rule(name)], &fake_context());
    let broken = "redundant_<wbr>derive_<wbr>more_<wbr>forward_<wbr>template";
    // The index table's "Lint" cell — the narrowest of the three
    // columns, and the one that motivated this.
    let cell = format!(
        r##"<td><a href="#/rule/redundant-derive-more-forward-template"><code>{broken}</code></a></td>"##,
    );
    assert!(
        html.contains(&cell),
        "the index table's lint name must break on `_` boundaries",
    );
    assert!(
        sidebar_list(&html).contains(&format!("<code>{broken}</code>")),
        "the sidebar's lint name must break on `_` boundaries",
    );
    // The heading's namespace prefix is a break opportunity of its
    // own: two adjacent inline boxes no more license a break between
    // them than `_` does inside the name.
    let heading = format!(
        r#"<span class="lint-prefix">perfectionist::</span><wbr><span class="lint-name">{broken}</span>"#,
    );
    assert!(
        html.contains(&heading),
        "the rule heading must break after the namespace and on `_` boundaries",
    );
}

#[test]
fn config_key_offers_a_line_break_after_every_underscore() {
    // The `dylint.toml` table header names the same identifier the
    // index table does, in a sentence just as liable to be squeezed
    // on a narrow viewport, so it breaks on `_` boundaries too. The
    // namespace stays whole: it carries no `_`.
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
    let html = crate::render::config::config_section(&config).into_string();
    assert!(
        html.contains("[&quot;perfectionist::demo_<wbr>rule&quot;]"),
        "the dylint.toml key must break on `_` boundaries, got: {html}",
    );
}
