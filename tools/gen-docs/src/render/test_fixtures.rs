//! The fixtures and readers the page's tests share.
//!
//! These are `pub(super)` rather than private because the groups that
//! use them are siblings, not descendants: a module's private items
//! reach its own descendants and nothing else.

use crate::model::{ConfigDoc, DefaultState, RenderContext, Rule};
use std::path::PathBuf;

pub(super) fn fake_rule(name: &str) -> Rule {
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

pub(super) fn fake_context() -> RenderContext<'static> {
    RenderContext {
        git_ref: "master",
        commit_sha: "0000000000000000000000000000000000000000",
        repo_url: "https://example.invalid/perfectionist",
    }
}

/// Locate the sidebar's `<ul>` and return the slice spanning
/// just its contents. The narrow-viewport CSS selector
/// `.nav-toggle[aria-expanded="true"] + .nav-sidebar` depends
/// on adjacent-sibling ordering, so tests that want to count
/// or inspect sidebar entries should do so against this slice
/// — not the whole page, where the index table and rule
/// articles emit substrings (`href="#/rule/*"`,
/// `<code>name</code>`, `id="/rule/*"`) that overlap
/// the sidebar's markup.
pub(super) fn sidebar_list(html: &str) -> &str {
    let open = r#"<ul class="nav-sidebar-list">"#;
    let close = "</ul>";
    let start = html.find(open).expect("sidebar <ul> not rendered") + open.len();
    let end = start
        + html[start..]
            .find(close)
            .expect("sidebar <ul> unterminated");
    &html[start..end]
}
