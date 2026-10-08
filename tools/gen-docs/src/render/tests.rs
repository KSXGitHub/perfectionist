//! What the page computes from the rules it is handed: the fragment a
//! lint name becomes, the line-break opportunities the name carries
//! wherever it is printed, and the one sidebar entry per rule.

use super::{anchor_for, render_page};
use crate::model::{ConfigDoc, DefaultState, RenderContext, Rule};
use std::path::PathBuf;

fn fake_rule(name: &str) -> Rule {
    Rule {
        namespaced: format!("perfectionist::{name}"),
        default_state: DefaultState::Active,
        short_desc: format!("demo rule {name}"),
        doc_markdown: "### What it does\nDoes a demo.".to_owned(),
        relative_source: PathBuf::from(format!("src/rules/{name}.rs")),
        config: ConfigDoc {
            key: format!("perfectionist::{name}"),
            fields: Vec::new(),
            custom_types: Vec::new(),
        },
    }
}

fn fake_context() -> RenderContext<'static> {
    RenderContext {
        git_ref: "master",
        commit_sha: "0000000000000000000000000000000000000000",
        repo_url: "https://example.invalid/perfectionist",
    }
}

/// Locate the sidebar's `<ul>` and return the slice spanning just its
/// contents. Tests that want to count or inspect sidebar entries should
/// do so against this slice — not the whole page, where the index table
/// and the rule articles emit substrings (`href="#/rule/*"`,
/// `<code>name</code>`, `id="/rule/*"`) that overlap the sidebar's
/// markup.
fn sidebar_list(html: &str) -> &str {
    let open = r#"<ul class="nav-sidebar-list">"#;
    let close = "</ul>";
    let start = html.find(open).expect("sidebar <ul> not rendered") + open.len();
    let end = start
        + html[start..]
            .find(close)
            .expect("sidebar <ul> unterminated");
    &html[start..end]
}

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
fn page_emits_one_sidebar_entry_per_rule_with_anchor_links() {
    let rules = [fake_rule("alpha"), fake_rule("beta_gamma")];
    let html = render_page(&rules, &fake_context());
    let sidebar = sidebar_list(&html);
    // One <li> per rule, scoped to the sidebar's <ul> so that
    // the index table's rows and the rule articles can't make
    // these assertions pass on their own.
    assert_eq!(sidebar.matches("<li>").count(), rules.len());
    assert!(sidebar.contains(r##"<li><a href="#/rule/alpha"><code>alpha</code></a></li>"##));
    // A multi-word rule name keeps its underscores (`beta_gamma`,
    // with `<wbr>` marking where the sidebar's narrow column may
    // break it) but its anchor is kebab-cased (`beta-gamma`).
    assert!(
        sidebar
            .contains(r##"<li><a href="#/rule/beta-gamma"><code>beta_<wbr>gamma</code></a></li>"##),
    );
    // The rule articles those anchors resolve to must exist
    // outside the sidebar slice, otherwise the sidebar links
    // dangle.
    assert!(html.contains(r#"id="/rule/alpha""#));
    assert!(html.contains(r#"id="/rule/beta-gamma""#));
}
