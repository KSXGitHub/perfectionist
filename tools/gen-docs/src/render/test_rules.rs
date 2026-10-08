//! A rule's own entry in the catalogue: the heading's permalink anchor,
//! and the Configuration block beneath it.

use super::render_page;
use super::test_fixtures::{fake_context, fake_rule};
use crate::model::{ConfigDoc, ConfigField, Optionality};

#[test]
fn lint_name_line_breaks_add_no_text() {
    // `<wbr>` is chosen over a zero-width space precisely because it
    // contributes no character: the name a reader selects, copies or
    // ctrl-Fs must still be the identifier they can paste into an
    // `#[expect(...)]`, underscores and all.
    let html = render_page(&[fake_rule("beta_gamma")], &fake_context());
    assert!(
        !html.contains('\u{200b}'),
        "a zero-width space would travel with the copied lint name; use <wbr>",
    );
    assert!(
        html.replace("<wbr>", "")
            .contains("<code>beta_gamma</code>"),
        "removing the break opportunities must leave the identifier intact",
    );
}

#[test]
fn rule_heading_emits_left_side_permalink_anchor() {
    let html = render_page(&[fake_rule("alpha")], &fake_context());
    assert!(
        html.contains(
            "<a class=\"rule-anchor\" href=\"#/rule/alpha\" \
             aria-label=\"Permalink to this rule\"></a>"
        ),
        "rule heading must emit a left-side permalink anchor to its own id",
    );
    let heading = r#"<h2><code><a class="rule-anchor""#;
    assert!(
        html.contains(heading),
        "the permalink anchor must be the first child of the rule heading's <code> (left side)",
    );
    // Tag-prefixed needles: the inlined stylesheet mentions these
    // as bare selectors, so a class-only search would match the CSS
    // in <head> before the body markup and scramble the ordering.
    let anchor_pos = html
        .find(r#"<a class="rule-anchor""#)
        .expect("rule-anchor missing");
    let code_pos = html
        .find(r#"<span class="lint-prefix""#)
        .expect("lint-prefix missing");
    let jump_pos = html
        .find(r#"<a class="rule-jump-link""#)
        .expect("jump link missing");
    assert!(
        anchor_pos < code_pos && code_pos < jump_pos,
        "permalink anchor must precede the rule name, which precedes the jump link",
    );
}

#[test]
fn config_section_keeps_both_quotes_around_the_dylint_toml_key() {
    // Regression guard: the `["<key>"]` TOML table header is built from
    // two quote-bearing string fragments in `config_section`. A
    // value-dropping raw-string rewrite of either (e.g. `r#"[""#`
    // mistyped as `r#"["#`, which parses as `[` and loses the quote)
    // silently renders `[key"]`. Assert both quotes survive around the
    // key. Only rules with at least one field render this header, so the
    // `fake_rule` fixtures elsewhere never covered it.
    let config = ConfigDoc {
        key: "perfectionist::demo".to_owned(),
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
        html.contains("[&quot;perfectionist::demo&quot;]"),
        "config heading must wrap the key in both quotes, got: {html}",
    );
}
