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
    /// Whether test-exclusive code is exempt: a function gated to test
    /// builds by `#[cfg(test)]` (or a compound predicate implying it),
    /// one declared inside a `#[test]` function, and every function in
    /// an integration-test (`tests/`) or benchmark (`benches/`) crate.
    /// Defaults to `true`; set `false` to hold test code to the same
    /// signature as production code.
    ///
    /// The exemption is off the rule's own rationale: a test helper's
    /// callers are test bodies holding literals, which would each have
    /// to write the `.to_owned()` the helper is being told to drop,
    /// and the copy that buys back is one a test never pays for. An
    /// example (`examples/`) is not covered — it is documentation that
    /// readers copy, so it is held to the library's standard.
    pub(super) test_code_exception: bool,
    /// Whether a build script — `build.rs`, or whatever `Cargo.toml`'s
    /// `build` key names — is exempt. Defaults to `true`; set `false`
    /// to check build scripts too.
    ///
    /// Same rationale as `test_code_exception`: a build script runs
    /// once per build, so the copy the rule exists to save is worth
    /// nothing there. Recognising one relies on Cargo's
    /// `build_script_*` crate-name convention.
    pub(super) build_script_exception: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            extra_conversion_methods: Vec::new(),
            ignore_conversion_methods: Vec::new(),
            test_code_exception: true,
            build_script_exception: true,
        }
    }
}

/// The resolved, lookup-ready form of [`Config`] held by the running
/// pass: the conversion names interned into a symbol set, and the two
/// exemption toggles carried verbatim.
pub(super) struct Resolved {
    pub(super) conversion_methods: BTreeSet<Symbol>,
    pub(super) test_code_exception: bool,
    pub(super) build_script_exception: bool,
}

impl Resolved {
    pub(super) fn from_config(config: Config) -> Self {
        Self {
            conversion_methods: resolve_symbol_set(
                DEFAULT_CONVERSION_METHODS,
                config.extra_conversion_methods,
                config.ignore_conversion_methods,
            ),
            test_code_exception: config.test_code_exception,
            build_script_exception: config.build_script_exception,
        }
    }
}
