//! User-facing configuration and the resolved in-memory state for
//! `perfectionist::unit_test_file_layout`.

use std::collections::BTreeSet;

use rustc_span::Symbol;

use super::CONFIG_KEY;

/// How inline test code is treated (the `inline_style` knob).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum InlineStyle {
    /// Every inline test item is flagged; all test code must move to an
    /// external `mod <name>;`. Matches pacquet's strict policy.
    ExternalOnly,
    /// Inline test code is allowed up to the configured budget; beyond
    /// that it must move to a file. Matches parallel-disk-usage's
    /// guidance. The default.
    ExternalWhenLong,
}

/// How external test files must be laid out on disk (the
/// `external_layout` knob).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ExternalLayout {
    /// `src/foo.rs`'s `mod bar;` must resolve to `src/foo/bar.rs`.
    ///
    /// ```text
    /// src/
    /// ├── lib.rs
    /// ├── foo.rs         declares  #[cfg(test)] mod tests;
    /// └── foo/
    ///     └── tests.rs   holds the test code
    /// ```
    Nested,
    /// Also accept the flattened `src/foo_bar.rs` form.
    ///
    /// ```text
    /// src/
    /// ├── lib.rs
    /// ├── foo.rs         declares  #[cfg(test)] mod tests;
    /// └── foo_tests.rs   holds the test code
    /// ``````
    Sibling,
    /// Accept whichever path Cargo loads; skip the layout check.
    Any,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// How inline test modules are handled. Defaults to
    /// `external_when_long`.
    inline_style: InlineStyle,
    /// Absolute cap, in lines, on the summed inline-test footprint of a
    /// file under `external_when_long`. Always active.
    inline_max_lines: usize,
    /// Optional relative cap: the share `inline_test_lines / file_lines`
    /// a file's inline tests may occupy under `external_when_long`.
    /// Accepted values are `0.0 <= x < 1.0`; omit the key to disable the
    /// relative cap (the default).
    inline_max_fraction_of_file: Option<f32>,
    /// How external test files must be laid out on disk. Defaults to
    /// `nested`.
    external_layout: ExternalLayout,
    /// Under `nested`, also flag a flattened `<parent>_<name>.rs` sibling
    /// left on disk for a module whose nested file already exists.
    /// Defaults to true.
    flag_unexpected_sibling: bool,
    /// Module names the inline-style footprint is scoped to. Empty (the
    /// default) counts every inline test item — `#[cfg(test)] mod`
    /// blocks of any name, `#[test] fn`s, and other `#[cfg(test)]`
    /// items. When non-empty, the budget is measured *only* over
    /// `#[cfg(test)] mod <name>` blocks whose `<name>` is listed; bare
    /// top-level test items (which have no module name) are then out of
    /// scope. Set this when a project keeps its inline tests in named
    /// modules and wants the budget to track those specifically.
    test_module_names: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            inline_style: InlineStyle::ExternalWhenLong,
            inline_max_lines: 50,
            inline_max_fraction_of_file: None,
            external_layout: ExternalLayout::Nested,
            flag_unexpected_sibling: true,
            test_module_names: Vec::new(),
        }
    }
}

pub(super) struct UnitTestFileLayout {
    pub(super) inline_style: InlineStyle,
    pub(super) inline_max_lines: usize,
    pub(super) inline_max_fraction_of_file: Option<f32>,
    pub(super) external_layout: ExternalLayout,
    pub(super) flag_unexpected_sibling: bool,
    /// Module names the inline-style footprint is restricted to. Empty
    /// means every `#[cfg(test)]` module qualifies.
    pub(super) test_module_names: BTreeSet<Symbol>,
}

impl UnitTestFileLayout {
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
                "perfectionist::unit_test_file_layout: `inline_max_fraction_of_file` must be in \
                 the range `0.0 <= x < 1.0`; got {fraction}. Omit the key to disable the relative \
                 cap.",
            );
        }
        Self {
            inline_style: config.inline_style,
            inline_max_lines: config.inline_max_lines,
            inline_max_fraction_of_file: config.inline_max_fraction_of_file,
            external_layout: config.external_layout,
            flag_unexpected_sibling: config.flag_unexpected_sibling,
            test_module_names: config
                .test_module_names
                .iter()
                .map(|name| Symbol::intern(name))
                .collect(),
        }
    }

    /// Whether a `#[cfg(test)]` module named `name` is in scope for the
    /// inline-style footprint. With the configured set empty (the
    /// default) every module qualifies.
    pub(super) fn module_name_in_scope(&self, name: Symbol) -> bool {
        self.test_module_names.is_empty() || self.test_module_names.contains(&name)
    }
}
