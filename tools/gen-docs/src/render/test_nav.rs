//! The collapsible navigation drawer: the one entry per rule inside the
//! sidebar.

use super::render_page;
use super::test_fixtures::{fake_context, fake_rule, sidebar_list};

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
