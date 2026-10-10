use super::{misaligned, template_sources};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn every_template_block_closes_where_it_opened() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources: Vec<PathBuf> = Vec::new();
    template_sources(&src, &mut sources);
    sources.sort();
    assert!(
        !sources.is_empty(),
        "no template found under {} -- has the macro been renamed?",
        src.display(),
    );

    let mut report = Vec::new();
    for path in &sources {
        let source = fs::read_to_string(path).expect("a listed source should be readable");
        for complaint in misaligned(&source) {
            report.push(format!("{}: {complaint}", path.display()));
        }
    }
    assert!(report.is_empty(), "{}", report.join("\n"));
}

#[test]
fn a_block_closing_where_it_opened_passes() {
    let source = "fn f() {\n    html! {\n        p {\n            \"text\"\n        }\n    }\n}";
    assert!(misaligned(source).is_empty());
}

#[test]
fn a_block_closing_short_of_its_opening_is_reported() {
    // The shape this exists to catch: the `{` rides the last attribute
    // line, so the child lands level with the attributes.
    let source = "    p\n        role=\"status\" {\n        \"text\"\n    }";
    let report = misaligned(source);
    assert_eq!(report.len(), 1, "{report:?}");
    assert!(report[0].contains("line 2"), "{report:?}");
    assert!(report[0].contains("line 4"), "{report:?}");
}

#[test]
fn an_else_arm_closes_and_opens_on_the_one_line() {
    let source = "@if x {\n    a\n} @else {\n    b\n}";
    assert!(misaligned(source).is_empty());
}

#[test]
fn a_pair_written_on_one_line_is_left_alone() {
    // `summary { "Configuration" }` opens and closes within the line,
    // so it is neither pushed nor popped.
    let source = "dl {\n    dt { code { \"key\" } }\n}";
    assert!(misaligned(source).is_empty());
}

#[test]
fn a_commented_brace_is_not_a_block() {
    let source = "p {\n    // a trailing brace in prose {\n    \"text\"\n}";
    assert!(misaligned(source).is_empty());
}
