use super::{Config, Resolved, validate};

#[test]
fn default_covers_alloc_and_exempts_nothing() {
    let resolved = Resolved::from_config(Config::default());
    assert!(resolved.also_alloc);
    assert!(resolved.skip_paths.is_empty());
}

#[test]
fn omitted_fields_fall_back_to_defaults() {
    let config: Config = toml::from_str(r#"skip_paths = ["::core::mem::transmute"]"#).unwrap();
    let resolved = Resolved::from_config(config);
    assert!(resolved.also_alloc);
    assert!(resolved.skip_paths.contains("::core::mem::transmute"));
}

#[test]
fn unknown_field_is_rejected() {
    assert!(toml::from_str::<Config>("nonsense = true").is_err());
}

#[test]
fn skip_paths_accepts_core_and_alloc_entries() {
    let config: Config =
        toml::from_str(r#"skip_paths = ["::core::mem::transmute", "::alloc::sync::Arc"]"#).unwrap();
    assert!(validate(&config).is_ok());
}

#[test]
fn skip_paths_rejects_a_missing_leading_root() {
    // Without the leading `::` the entry is a relative path, which is not
    // the shape the lint matches against.
    let config: Config = toml::from_str(r#"skip_paths = ["core::mem::transmute"]"#).unwrap();
    assert!(validate(&config).is_err());
}

#[test]
fn skip_paths_rejects_a_path_the_rule_could_never_flag() {
    // Another crate entirely — the rule only ever flags `core` / `alloc`.
    let other: Config = toml::from_str(r#"skip_paths = ["::rayon::iter"]"#).unwrap();
    assert!(validate(&other).is_err());
    // The crate root on its own names no item under it.
    let bare: Config = toml::from_str(r#"skip_paths = ["::core"]"#).unwrap();
    assert!(validate(&bare).is_err());
    // An empty segment is a typo'd `::`, not a path.
    let empty_segment: Config = toml::from_str(r#"skip_paths = ["::core::"]"#).unwrap();
    assert!(validate(&empty_segment).is_err());
}
