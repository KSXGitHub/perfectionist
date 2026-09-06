//! Configuration for `excessive_cognitive_complexity`.

const CONFIG_KEY: &str = "perfectionist::excessive_cognitive_complexity";

/// The threshold SonarSource ships for every language its Cognitive
/// Complexity metric covers.
const DEFAULT_MAX_COMPLEXITY: usize = 15;

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// The highest cognitive complexity a function may have without
    /// being flagged. Defaults to `15`.
    pub(super) max_complexity: usize,
    /// Whether test code is left alone: functions inside a
    /// `#[cfg(test)]` module, `#[test]` functions, and everything in
    /// an integration-test or benchmark target. Defaults to `false`,
    /// so a test is held to the same limit as the code it exercises.
    pub(super) exempt_tests: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_complexity: DEFAULT_MAX_COMPLEXITY,
            exempt_tests: false,
        }
    }
}

impl Config {
    pub(super) fn load() -> Self {
        dylint_linting::config_or_default(CONFIG_KEY)
    }
}
