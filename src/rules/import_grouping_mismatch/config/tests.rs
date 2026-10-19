use super::{CfgBlockHandling, Config, ReexportGrouping, Style, default_order};
use pipe_trait::Pipe;

#[test]
fn style_values_deserialize() {
    assert_eq!(
        "style = \"multi_block\"\nreexports = \"grouped\""
            .pipe(toml::from_str::<Config>)
            .unwrap()
            .style,
        Style::MultiBlock,
    );
    assert_eq!(
        "style = \"single_block\"\nreexports = \"grouped\""
            .pipe(toml::from_str::<Config>)
            .unwrap()
            .style,
        Style::SingleBlock,
    );
}

/// `style` and `reexports` are both required (bare fields, no
/// `serde(default)`), so a table that omits either fails to deserialize
/// rather than defaulting to a layout.
#[test]
fn missing_required_fields_are_an_error() {
    assert!("".pipe(toml::from_str::<Config>).is_err());
    assert!(r#"order = ["std"]"#.pipe(toml::from_str::<Config>).is_err());
    // `style` set but `reexports` missing is still an error.
    assert!(r#"style = "multi_block""#.pipe(toml::from_str::<Config>).is_err());
    // `reexports` set but `style` missing is still an error.
    assert!(r#"reexports = "grouped""#.pipe(toml::from_str::<Config>).is_err());
}

/// With `style` and `reexports` set, the remaining knobs fall back to
/// their per-field defaults when absent.
#[test]
fn optional_fields_default_when_required_are_set() {
    let config = "style = \"multi_block\"\nreexports = \"grouped\""
        .pipe(toml::from_str::<Config>)
        .unwrap();
    assert_eq!(config.order, default_order());
    assert_eq!(config.cfg_block_handling, CfgBlockHandling::Trailing);
}

/// There is no neutral `preserve` value; an unrecognised style is a
/// hard deserialisation error rather than a silent no-op (the
/// `reexports` field is valid here, so the style is the only fault).
#[test]
fn unknown_style_is_rejected() {
    assert!(
        "style = \"preserve\"\nreexports = \"grouped\""
            .pipe(toml::from_str::<Config>)
            .is_err(),
    );
}

/// The three re-export grouping modes round-trip from their snake_case
/// spellings, and an unrecognised one is rejected.
#[test]
fn reexports_values_deserialize() {
    let parse = |value: &str| {
        format!("style = \"multi_block\"\nreexports = \"{value}\"")
            .pipe_deref(toml::from_str::<Config>)
            .map(|config| config.reexports)
    };
    assert_eq!(parse("by_path").unwrap(), ReexportGrouping::ByPath);
    assert_eq!(parse("grouped").unwrap(), ReexportGrouping::Grouped);
    assert_eq!(parse("split").unwrap(), ReexportGrouping::Split);
    assert!(parse("separate").is_err());
}

/// The private-import ranks shift down by the size of the leading
/// re-export region: none for `by_path`, one `grouped` block, two
/// `split` blocks.
#[test]
fn reexport_block_offset_tracks_grouping() {
    let with = |value: &str| {
        format!("style = \"multi_block\"\nreexports = \"{value}\"")
            .pipe_deref(toml::from_str::<Config>)
            .unwrap()
            .reexport_block_offset()
    };
    assert_eq!(with("by_path"), 0);
    assert_eq!(with("grouped"), 1);
    assert_eq!(with("split"), 2);
}
