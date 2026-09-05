//! Configuration for `needless_borrowed_parameters`: which method
//! names count as a conversion to the owned form, and which bodies of
//! code the rule stays out of.

use crate::common::resolve_symbol_set;
use rustc_span::Symbol;
use std::collections::BTreeSet;

/// Method names that count as "convert the borrowed parameter to its
/// owned form". `into` is included for `param.into()`; the `from`
/// free-function shape (`String::from(param)`) is recognised
/// unconditionally and is not part of this set.
const DEFAULT_CONVERSION_METHODS: &[&str] = &[
    "to_owned",
    "to_string",
    "to_path_buf",
    "to_vec",
    "to_os_string",
    "clone",
    "into",
];

/// The user-facing configuration shape, deserialised from the
/// `["perfectionist::needless_borrowed_parameters"]` table of
/// `dylint.toml`.
#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// Additional method names that count as a conversion of the
    /// borrowed parameter to its owned form. Merged with the built-in
    /// defaults (`["to_owned", "to_string", "to_path_buf", "to_vec",
    /// "to_os_string", "clone", "into"]`); empty by default. A
    /// flagged conversion must still actually produce the owned
    /// counterpart of the parameter's type, so listing an unrelated
    /// method here never widens the lint beyond owned-producing calls.
    extra_conversion_methods: Vec<String>,
    /// Method names to drop from the conversion set, even if they
    /// appear in the built-in defaults or in
    /// `extra_conversion_methods`. Empty by default; checked after the
    /// merge with the built-ins, so this knob always wins.
    ignore_conversion_methods: Vec<String>,
    /// Whether test code is exempt: anything gated to test builds by
    /// `#[cfg(test)]` (or a compound predicate implying it), anything
    /// inside a `#[test]` function, and every `tests/` or `benches/`
    /// crate. An `examples/` crate is not covered. Defaults to `true`.
    pub(super) exempt_tests: bool,
    /// Whether a build script — `build.rs`, or whatever `Cargo.toml`'s
    /// `build` key names — is exempt. Defaults to `true`.
    pub(super) exempt_build_scripts: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            extra_conversion_methods: Vec::new(),
            ignore_conversion_methods: Vec::new(),
            exempt_tests: true,
            exempt_build_scripts: true,
        }
    }
}

/// The resolved, lookup-ready form of [`Config`] held by the running
/// pass: the conversion names interned into a symbol set, and the two
/// exemption toggles carried verbatim.
pub(super) struct Resolved {
    pub(super) conversion_methods: BTreeSet<Symbol>,
    pub(super) exempt_tests: bool,
    pub(super) exempt_build_scripts: bool,
}

impl Resolved {
    pub(super) fn from_config(config: Config) -> Self {
        Self {
            conversion_methods: resolve_symbol_set(
                DEFAULT_CONVERSION_METHODS,
                config.extra_conversion_methods,
                config.ignore_conversion_methods,
            ),
            exempt_tests: config.exempt_tests,
            exempt_build_scripts: config.exempt_build_scripts,
        }
    }
}
