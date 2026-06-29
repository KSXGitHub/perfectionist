use super::{CfgBlockHandling, Config, ReexportGrouping, Style, default_order};

#[test]
fn style_values_deserialize() {
    assert_eq!(
        toml::from_str::<Config>(r#"style = "multi_block""#)
            .unwrap()
            .style,
        Style::MultiBlock,
    );
    assert_eq!(
        toml::from_str::<Config>(r#"style = "single_block""#)
            .unwrap()
            .style,
        Style::SingleBlock,
    );
}

#[test]
fn missing_style_is_an_error() {
    // `style` is required (bare `Style`, no `serde(default)`), so a
    // table that omits it fails to deserialize rather than defaulting
    // to a layout — even when another knob is present.
    assert!(toml::from_str::<Config>("").is_err());
    assert!(toml::from_str::<Config>(r#"order = ["std"]"#).is_err());
}

#[test]
fn other_fields_default_when_style_is_set() {
    // Only `style` is mandatory; the remaining knobs fall back to
    // their per-field defaults when absent.
    let config = toml::from_str::<Config>(r#"style = "multi_block""#).unwrap();
    assert_eq!(config.order, default_order());
    assert_eq!(config.cfg_block_handling, CfgBlockHandling::Trailing);
    assert_eq!(config.reexports, ReexportGrouping::Grouped);
}

#[test]
fn unknown_style_is_rejected() {
    // There is no neutral `preserve` value; an unrecognised style is
    // a hard deserialisation error rather than a silent no-op.
    assert!(toml::from_str::<Config>(r#"style = "preserve""#).is_err());
}

#[test]
fn reexports_values_deserialize() {
    // The three re-export grouping modes round-trip from their
    // snake_case spellings, and an unrecognised one is rejected.
    let parse = |value: &str| {
        toml::from_str::<Config>(&format!("style = \"multi_block\"\nreexports = \"{value}\""))
            .map(|config| config.reexports)
    };
    assert_eq!(parse("by_path").unwrap(), ReexportGrouping::ByPath);
    assert_eq!(parse("grouped").unwrap(), ReexportGrouping::Grouped);
    assert_eq!(parse("split").unwrap(), ReexportGrouping::Split);
    assert!(parse("separate").is_err());
}

#[test]
fn reexport_block_offset_tracks_grouping() {
    // The private-import ranks shift down by the size of the leading
    // re-export region: none for `by_path`, one `grouped` block, two
    // `split` blocks.
    let with = |value: &str| {
        toml::from_str::<Config>(&format!("style = \"multi_block\"\nreexports = \"{value}\""))
            .unwrap()
            .reexport_block_offset()
    };
    assert_eq!(with("by_path"), 0);
    assert_eq!(with("grouped"), 1);
    assert_eq!(with("split"), 2);
}
