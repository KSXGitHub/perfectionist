//! Configuration for `mutating_command_builder`: whether the lint
//! stays silent in a crate that has not loaded `command-extra`.

/// The user-facing configuration shape, deserialised from the
/// `["perfectionist::mutating_command_builder"]` table of
/// `dylint.toml`.
#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Whether to stay silent in a crate that has not loaded
    /// `command-extra`. Defaults to `true`: without the crate the
    /// suggested method does not exist, so the diagnostic would name
    /// something the author cannot write. Set it to `false` in a
    /// workspace that adds the dependency per-crate and wants the
    /// lint to say where it is still missing.
    pub(super) require_command_extra_dependency: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            require_command_extra_dependency: true,
        }
    }
}
