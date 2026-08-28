//! User-facing configuration and the resolved in-memory state for
//! `perfectionist::excessive_inline_tests`.

use super::CONFIG_KEY;

/// How inline test code is treated (the `inline_style` knob).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum InlineStyle {
    /// Every inline test item is flagged; all test code must move to an
    /// external `mod <name>;`, whatever its length.
    ExternalOnly,
    /// Inline test code is allowed up to the configured budget; beyond
    /// that it must move to an external `mod <name>;`.
    ExternalWhenLong,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// How inline test code is handled. Defaults to
    /// `external_when_long`.
    inline_style: InlineStyle,
    /// Absolute cap, in lines, on the summed inline-test footprint of a
    /// file under `external_when_long`; always active. Defaults to `50`.
    inline_max_lines: usize,
    /// Optional relative cap: the share `inline_test_lines / file_lines`
    /// a file's inline tests may occupy under `external_when_long`.
    /// Accepted values are `0.0 <= x < 1.0`; omit the key to disable the
    /// relative cap (the default).
    inline_max_fraction_of_file: Option<f32>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            inline_style: InlineStyle::ExternalWhenLong,
            inline_max_lines: 50,
            inline_max_fraction_of_file: None,
        }
    }
}

pub(super) struct ExcessiveInlineTests {
    pub(super) inline_style: InlineStyle,
    pub(super) inline_max_lines: usize,
    pub(super) inline_max_fraction_of_file: Option<f32>,
}

impl ExcessiveInlineTests {
    pub(super) fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        // Reject an out-of-range relative cap rather than silently
        // clamping: "disabled" is expressed by omitting the key, and
        // any in-range fraction is strictly below 1.0, so a value at or
        // above the ceiling could never fire and would only confuse.
        if let Some(fraction) = config.inline_max_fraction_of_file
            && !(0.0..1.0).contains(&fraction)
        {
            panic!(
                "perfectionist::excessive_inline_tests: `inline_max_fraction_of_file` must be in \
                 the range `0.0 <= x < 1.0`; got {fraction}. Omit the key to disable the relative \
                 cap.",
            );
        }
        Self {
            inline_style: config.inline_style,
            inline_max_lines: config.inline_max_lines,
            inline_max_fraction_of_file: config.inline_max_fraction_of_file,
        }
    }
}
