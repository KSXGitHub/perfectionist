//! The `[perfectionist]` table's `enable` / `disable` arrays, from the
//! TOML a user writes through to the state a rule registers under.
//!
//! Every case resolves a map of its own. Writing `GLOBAL_OVERRIDES`
//! instead would hand the whole binary one of these fixtures: the lock
//! takes a value once per process, and a rule that read it afterwards
//! would register against whichever case got there first.

use super::{DefaultState, GlobalConfig, resolve_overrides, resolved_state_from};
use std::collections::HashMap;

/// The `[perfectionist]` table a user wrote, parsed the way
/// `dylint_linting::config_or_default` parses it.
fn overrides(table: &str) -> HashMap<String, DefaultState> {
    let config: GlobalConfig = toml::from_str(table).expect("the table should deserialise");
    resolve_overrides(&config)
}

/// A bare string and an inline `{ name, reason }` table are the two
/// spellings one array mixes, and `reason` is decorative: both key the
/// map by the rule name alone.
#[test]
fn either_selector_spelling_keys_by_name() {
    let resolved = overrides(
        r#"
            enable = ["alpha", { name = "beta", reason = "why we want it" }]
            disable = ["gamma", { name = "delta", reason = "why we don't" }]
        "#,
    );
    assert_eq!(resolved.get("alpha"), Some(&DefaultState::Active));
    assert_eq!(resolved.get("beta"), Some(&DefaultState::Active));
    assert_eq!(resolved.get("gamma"), Some(&DefaultState::Inactive));
    assert_eq!(resolved.get("delta"), Some(&DefaultState::Inactive));
}

/// An absent array is an empty one, so a table that names nothing
/// leaves every rule on its own baseline.
#[test]
fn an_empty_table_overrides_nothing() {
    assert!(overrides("").is_empty());
}

/// One name twice under one array agrees with itself, so it is not the
/// contradiction the panic is for.
#[test]
fn a_name_repeated_under_one_array_is_accepted() {
    let resolved = overrides(r#"disable = ["alpha", { name = "alpha" }, "alpha"]"#);
    assert_eq!(resolved.get("alpha"), Some(&DefaultState::Inactive));
}

/// Both arrays naming one rule asks for it on and off at once. The
/// panic is what keeps the run from picking a direction the user never
/// chose, so no resolution order between the two arrays is observable.
#[test]
#[should_panic(expected = "listed under both `enable` and `disable`")]
fn a_name_under_both_arrays_panics() {
    overrides(
        r#"
            enable = ["alpha"]
            disable = ["alpha"]
        "#,
    );
}

/// A rule the table does not name keeps the baseline its
/// `register_pass` supplies, and that baseline differs per rule — the
/// off-by-default ones pass `Inactive`.
#[test]
fn an_unnamed_rule_keeps_its_baseline() {
    let resolved = overrides(r#"enable = ["alpha"]"#);
    for baseline in [DefaultState::Active, DefaultState::Inactive] {
        assert_eq!(
            resolved_state_from(&resolved, "unnamed", baseline),
            baseline,
        );
    }
}

/// A rule the table does name takes the state the table asked for,
/// whichever baseline it would otherwise have had.
#[test]
fn a_named_rule_takes_the_configured_state() {
    let resolved = overrides(
        r#"
            enable = ["alpha"]
            disable = ["beta"]
        "#,
    );
    for baseline in [DefaultState::Active, DefaultState::Inactive] {
        assert_eq!(
            resolved_state_from(&resolved, "alpha", baseline),
            DefaultState::Active,
        );
        assert_eq!(
            resolved_state_from(&resolved, "beta", baseline),
            DefaultState::Inactive,
        );
    }
}
