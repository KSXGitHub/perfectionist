use super::{promote_headings, render_index_md, render_rule_md, rule_file_name};
use crate::model::{
    ConfigDoc, ConfigField, DefaultState, EnumVariant, Optionality, Rule, StructField, TypeDoc,
    TypeKind,
};
use std::path::PathBuf;

fn fake_rule() -> Rule {
    Rule {
        namespaced: "perfectionist::demo_rule".to_owned(),
        default_state: DefaultState::Active,
        short_desc: "demo rule used in tests".to_owned(),
        doc_markdown: "### What it does\nDoes a demo.".to_owned(),
        relative_source: PathBuf::from("src/rules/demo_rule.rs"),
        config: ConfigDoc {
            key: "perfectionist::demo_rule".to_owned(),
            fields: Vec::new(),
            custom_types: Vec::new(),
        },
    }
}

#[test]
fn rule_file_name_strips_namespace() {
    let rule = fake_rule();
    assert_eq!(rule_file_name(&rule), "demo_rule.md");
}

#[test]
fn rule_md_includes_header_state_and_short_desc() {
    let md = render_rule_md(&fake_rule(), "../");
    assert!(md.contains("# `perfectionist::demo_rule`\n"));
    assert!(md.contains("**Default state:** `active`"));
    assert!(md.contains("> demo rule used in tests"));
    assert!(md.ends_with('\n'));
    assert!(!md.ends_with("\n\n"));
}

#[test]
fn rule_md_renders_inactive_state_for_opt_in_rules() {
    let mut rule = fake_rule();
    rule.default_state = DefaultState::Inactive;
    let md = render_rule_md(&rule, "../");
    assert!(md.contains("**Default state:** `inactive`"));
}

#[test]
fn rule_md_with_no_config_prints_none_section() {
    let md = render_rule_md(&fake_rule(), "../");
    assert!(md.contains("## Configuration"));
    assert!(md.contains("None."));
}

#[test]
fn rule_md_with_config_lists_fields_and_types() {
    let mut rule = fake_rule();
    rule.config = ConfigDoc {
        key: "perfectionist::demo_rule".to_owned(),
        fields: vec![
            // `optionality` is set directly on these fixtures; the
            // real extractor derives it syntactically from each
            // field's type and serde attributes (see `extract::config`).
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
        custom_types: vec![TypeDoc {
            name: "Style".to_owned(),
            doc_markdown: "Style enum.".to_owned(),
            kind: TypeKind::Enum {
                variants: vec![
                    EnumVariant {
                        rust_name: "Preserve".to_owned(),
                        serialized: "preserve".to_owned(),
                        doc_markdown: "Leave it alone.".to_owned(),
                    },
                    EnumVariant {
                        rust_name: "Same".to_owned(),
                        serialized: "Same".to_owned(),
                        doc_markdown: String::new(),
                    },
                ],
            },
        }],
    };
    let md = render_rule_md(&rule, "../");
    // The intro note uses the mandatory-branch wording because a
    // required field is present.
    assert!(
        md.contains("A field marked mandatory must be set;"),
        "got:\n{md}",
    );
    // Every heading is the item's bare name; the type, the
    // optionality and the kind ride in the metadata list below it.
    assert!(
        md.contains("### `style`\n\n- _Type:_ `Style`\n- _Mandatory_\n\n"),
        "got:\n{md}",
    );
    assert!(
        !md.contains("### `style`\n\n- _Type:_ `Style`\n- _Optional_"),
        "got:\n{md}",
    );
    assert!(
        md.contains("### `extras`\n\n- _Type:_ `[string]`\n- _Optional_\n\n"),
        "got:\n{md}",
    );
    assert!(md.contains("Pick a style."));
    assert!(md.contains("#### `Style`\n"), "got:\n{md}");
    assert!(!md.contains("(enum)"), "got:\n{md}");
    // Renamed variant carries the Rust-side annotation below the
    // heading.
    assert!(
        md.contains("##### `\"preserve\"`\n\n- _Rust:_ `Preserve`\n\n"),
        "got:\n{md}",
    );
    // Same-named variant doesn't.
    assert!(md.contains(r#"##### `"Same"`"#));
    assert!(!md.contains("_Rust:_ `Same`"));
    // Empty doc fall-back.
    assert!(md.contains("*Undocumented.*"));
}

#[test]
fn rule_md_struct_type_fields_carry_a_type_bullet() {
    // The struct branch of the Types section follows the same
    // heading convention as the enum branch: the field name alone
    // in the heading, its TOML type in the metadata list.
    let mut rule = fake_rule();
    rule.config = ConfigDoc {
        key: "perfectionist::demo_rule".to_owned(),
        fields: vec![ConfigField {
            name: "hosts".to_owned(),
            type_label: "[HostEntry]".to_owned(),
            doc_markdown: "Known hosts.".to_owned(),
            optionality: Optionality::Optional,
        }],
        custom_types: vec![TypeDoc {
            name: "HostEntry".to_owned(),
            doc_markdown: "One host.".to_owned(),
            kind: TypeKind::Struct {
                fields: vec![StructField {
                    name: "host".to_owned(),
                    type_label: "string".to_owned(),
                    doc_markdown: "The hostname.".to_owned(),
                }],
            },
        }],
    };
    let md = render_rule_md(&rule, "../");
    assert!(md.contains("#### `HostEntry`\n"), "got:\n{md}");
    assert!(!md.contains("(struct)"), "got:\n{md}");
    assert!(
        md.contains("##### `host`\n\n- _Type:_ `string`\n\nThe hostname.\n"),
        "got:\n{md}",
    );
}

#[test]
fn index_md_renders_bullet_list() {
    let mut rule = fake_rule();
    rule.short_desc = "uses | inside".to_owned();
    let index = render_index_md(std::slice::from_ref(&rule));
    // Each entry spans two lines: the link/state line, then a
    // blank line, then the indented short-description
    // continuation paragraph.
    assert!(index.contains("- [`demo_rule`](./demo_rule.md) (default: `active`).\n"));
    assert!(index.contains("\n  uses | inside\n"));
    // The bullet-list form needs no `|` escaping (unlike a
    // table) — the pipe in the description appears raw.
    assert!(!index.contains(r"\|"));
}

#[test]
fn rule_md_starts_with_generated_banner() {
    let md = render_rule_md(&fake_rule(), "../");
    assert!(md.starts_with("<!-- Generated by `gen-docs write-md`"));
}

#[test]
fn index_md_starts_with_generated_banner() {
    let index = render_index_md(&[fake_rule()]);
    assert!(index.starts_with("<!-- Generated by `gen-docs write-md`"));
}

#[test]
fn doc_markdown_headings_are_promoted_one_level() {
    // The rule title is h1 and `## Configuration` is h2. A
    // source-level `### What it does` should render as `## What
    // it does` so prose sections sit parallel to Configuration
    // instead of at the same depth as h3 config fields.
    let mut rule = fake_rule();
    rule.doc_markdown =
        "### What it does\nDoes a demo.\n\n### Example\n```text\n### not a heading\n```".to_owned();
    let md = render_rule_md(&rule, "../");
    assert!(md.contains("\n## What it does\n"), "got:\n{md}");
    assert!(md.contains("\n## Example\n"), "got:\n{md}");
    // The `###` line inside the fenced code block must stay
    // untouched, since it's example text, not a heading.
    assert!(md.contains("### not a heading"), "got:\n{md}");
    // And the original h3 must not survive at top level.
    assert!(!md.contains("\n### What it does\n"), "got:\n{md}");
}

#[test]
fn promote_headings_leaves_h1_alone() {
    // Demoting h1 would produce `# ` with no content. Source
    // markdown shouldn't have h1 anyway (the rule title takes
    // that slot), but if it appears, leave it intact rather
    // than mangling it.
    let out = promote_headings("# Title\nBody.\n");
    assert_eq!(out, "# Title\nBody.\n");
}

#[test]
fn promote_headings_caps_at_h6() {
    // h6 promoted would be h5; nothing past h6 to worry about.
    // A line of seven `#` is not a heading per CommonMark.
    let out = promote_headings("####### not a heading\n");
    assert_eq!(out, "####### not a heading\n");
}

#[test]
fn promote_headings_distinguishes_fence_markers() {
    // A triple-backtick fence stays open through a `~~~` line
    // in its body — `~~~` is the wrong marker, so it's not a
    // close — and any `## inside` between them must not be
    // treated as a heading. Per CommonMark §4.5, fence char
    // and length both matter for the close.
    let input = "```rust\n## inside backtick fence\n~~~\nstill inside\n```\n## outside\n";
    let out = promote_headings(input);
    // The body lines pass through verbatim.
    assert!(
        out.contains("## inside backtick fence\n"),
        "body should be untouched: {out}",
    );
    assert!(out.contains("~~~\n"), "tilde line should pass: {out}");
    // The line after the close-fence is outside; promote it.
    assert!(out.contains("# outside\n"), "outside promoted: {out}");
    assert!(
        !out.contains("\n## outside\n"),
        "outside should be promoted, not left at h2: {out}",
    );
}

#[test]
fn promote_headings_treats_indented_fence_marker_as_indented_code() {
    // Per CommonMark §4.5, a fence opener may be indented at
    // most 3 spaces; with 4+ spaces it's an indented code
    // block, not a fence. The previous implementation
    // trimmed *all* leading whitespace before checking for
    // backticks, so a `    ```` line opened a phantom fence
    // and any subsequent heading line was suppressed.
    let input = "    ```\n    ## inside indented code\n    ```\n## outside\n";
    let out = promote_headings(input);
    // The indented `` ``` `` lines pass through verbatim.
    assert!(
        out.contains("    ```\n"),
        "indented backticks should not open a fence: {out}",
    );
    // The 4-space-indented heading is inside an indented
    // code block, so it stays untouched too.
    assert!(
        out.contains("    ## inside indented code\n"),
        "indented heading should not be promoted: {out}",
    );
    // The unindented heading outside is promoted normally.
    assert!(
        out.contains("\n# outside\n"),
        "trailing heading should be promoted: {out}",
    );
}

#[test]
fn promote_headings_rejects_info_string_as_close_fence() {
    // Per CommonMark §4.5, the close fence cannot carry an
    // info string. A nested `` ```rust `` line inside a wider
    // outer fence is body, not a close.
    let input = "````\n## inside\n```rust\n## still inside\n````\n## outside\n";
    let out = promote_headings(input);
    assert!(
        out.contains("## inside\n"),
        "body heading should not be promoted: {out}",
    );
    assert!(
        out.contains("## still inside\n"),
        "heading after the bogus close should still be body: {out}",
    );
    // The real close (4 backticks alone) ends the fence.
    assert!(
        out.contains("# outside\n"),
        "outside should be promoted: {out}",
    );
}

#[test]
fn promote_headings_requires_matching_close_length() {
    // Per CommonMark §4.5, a fence opened with 4 backticks
    // closes only on ≥ 4 backticks. A 3-backtick line inside
    // is body, not a close.
    let input = "````\n```\n## inside long fence\n````\n## outside\n";
    let out = promote_headings(input);
    assert!(
        out.contains("## inside long fence\n"),
        "inside long fence should pass: {out}",
    );
    assert!(out.contains("# outside\n"), "outside promoted: {out}");
}
