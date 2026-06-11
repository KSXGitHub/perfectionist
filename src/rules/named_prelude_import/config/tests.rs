use super::*;

#[test]
fn default_recognises_prelude() {
    let resolved = Resolved::from_config(Config::default());
    assert!(resolved.prelude_segment_names.contains("prelude"));
    assert!(resolved.allowed_paths.is_empty());
}

#[test]
fn omitted_fields_fall_back_to_defaults() {
    let config: Config = toml::from_str(r#"allowed_paths = ["::crate::prelude"]"#).unwrap();
    let resolved = Resolved::from_config(config);
    assert!(resolved.prelude_segment_names.contains("prelude"));
    assert!(resolved.allowed_paths.contains("::crate::prelude"));
}

#[test]
fn unknown_field_is_rejected() {
    assert!(toml::from_str::<Config>("nonsense = true").is_err());
}

#[test]
fn allowed_paths_must_end_with_a_prelude_segment() {
    // Ends with the default prelude segment `prelude` — accepted.
    let ok: Config = toml::from_str(r#"allowed_paths = ["::crate::prelude"]"#).unwrap();
    assert!(validate(&ok).is_ok());
    // Ends with a non-prelude segment — rejected.
    let bad: Config = toml::from_str(r#"allowed_paths = ["::crate::foo"]"#).unwrap();
    assert!(validate(&bad).is_err());
}

#[test]
fn allowed_paths_validation_follows_prelude_segment_names() {
    // With a custom prelude name, an `::...::api` entry is accepted and an
    // `::...::prelude` entry (no longer a prelude) is rejected.
    let cfg: Config =
        toml::from_str("prelude_segment_names = [\"api\"]\nallowed_paths = [\"::crate::api\"]")
            .unwrap();
    assert!(validate(&cfg).is_ok());
    let stale: Config =
        toml::from_str("prelude_segment_names = [\"api\"]\nallowed_paths = [\"::crate::prelude\"]")
            .unwrap();
    assert!(validate(&stale).is_err());
}

#[test]
fn empty_or_root_only_allowed_path_is_rejected() {
    // No nameable final segment — rejected rather than silently matching
    // nothing.
    let bad: Config = toml::from_str(r#"allowed_paths = ["::"]"#).unwrap();
    assert!(validate(&bad).is_err());
}
