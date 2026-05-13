//! Configuration for `unicode_ellipsis_in_panic_messages`. Owns the
//! user-facing `Config` shape, the curated default macro / method
//! lists, and the in-memory `UnicodeEllipsisInPanicMessages` state the
//! late pass holds.

use std::collections::BTreeSet;

use rustc_span::Symbol;

use crate::common::merge_string_allowlist;

const CONFIG_KEY: &str = "perfectionist::unicode_ellipsis_in_panic_messages";

const DEFAULT_MACROS: &[&str] = &[
    "panic",
    "unimplemented",
    "todo",
    "unreachable",
    "debug_unreachable",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
];

const DEFAULT_METHODS: &[&str] = &["expect", "expect_err"];

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Additional macros whose call site should be scanned for
    /// the flagged characters. Merged with the built-in defaults
    /// (the standard panic and assertion macros — `panic`,
    /// `unimplemented`, `todo`, `unreachable`, `debug_unreachable`,
    /// and the `assert*` family); empty by default. Use this to
    /// add project-specific assertion-shaped macros without having
    /// to re-state the standard ones.
    extra_macros: Vec<String>,
    /// Macros to drop from the scanned set, even if they appear in
    /// the built-in defaults or in `extra_macros`. Empty by
    /// default; checked after the merge with the built-ins, so
    /// this knob always wins. Use it when a project deliberately
    /// uses `…` in one of the default macros.
    ignore_macros: Vec<String>,
    /// Additional method names on `Option` / `Result` whose first
    /// argument is the panic message. Merged with the built-in
    /// defaults (`expect`, `expect_err`); empty by default.
    extra_methods: Vec<String>,
    /// Methods to drop from the scanned set, even if they appear
    /// in the built-in defaults or in `extra_methods`. Empty by
    /// default; checked after the merge with the built-ins, so
    /// this knob always wins.
    ignore_methods: Vec<String>,
    /// Extra characters to flag alongside U+2026, in the same spirit
    /// as `unicode_ellipsis_in_comments.also_flag`. Empty by default.
    also_flag: Vec<char>,
}

pub(super) struct UnicodeEllipsisInPanicMessages {
    pub(super) flagged_chars: Vec<char>,
    pub(super) macros: BTreeSet<Symbol>,
    pub(super) methods: BTreeSet<Symbol>,
}

impl UnicodeEllipsisInPanicMessages {
    pub(super) fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let mut flagged_chars = vec!['\u{2026}'];
        for character in config.also_flag {
            if !flagged_chars.contains(&character) {
                flagged_chars.push(character);
            }
        }
        Self {
            flagged_chars,
            macros: intern_symbols(merge_string_allowlist(
                DEFAULT_MACROS,
                config.extra_macros,
                config.ignore_macros,
            )),
            methods: intern_symbols(merge_string_allowlist(
                DEFAULT_METHODS,
                config.extra_methods,
                config.ignore_methods,
            )),
        }
    }
}

fn intern_symbols(names: BTreeSet<String>) -> BTreeSet<Symbol> {
    names.iter().map(|name| Symbol::intern(name)).collect()
}
