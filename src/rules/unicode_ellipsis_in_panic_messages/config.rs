//! Configuration for `unicode_ellipsis_in_panic_messages`. Owns the
//! user-facing `Config` shape, the curated default macro / method
//! lists, and the in-memory `UnicodeEllipsisInPanicMessages` state the
//! late pass holds.

use rustc_span::Symbol;

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

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Macros whose call site should be scanned for the flagged
    /// characters. Defaults to the standard panic and assertion
    /// macros (`panic`, `unimplemented`, `todo`, `unreachable`,
    /// `debug_unreachable`, and the `assert*` family). Override to
    /// add project-specific assertion-shaped macros, or to narrow
    /// the set when a project deliberately uses `…` in one of them.
    macros: Vec<String>,
    /// Method names on `Option` / `Result` whose first argument is
    /// the panic message. Defaults to `expect` and `expect_err`.
    methods: Vec<String>,
    /// Extra characters to flag alongside U+2026, in the same spirit
    /// as `unicode_ellipsis_in_comments.also_flag`. Empty by default.
    also_flag: Vec<char>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            macros: DEFAULT_MACROS
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            methods: DEFAULT_METHODS
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            also_flag: Vec::new(),
        }
    }
}

pub(super) struct UnicodeEllipsisInPanicMessages {
    pub(super) flagged_chars: Vec<char>,
    pub(super) macros: Vec<Symbol>,
    pub(super) methods: Vec<Symbol>,
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
            macros: config
                .macros
                .iter()
                .map(|name| Symbol::intern(name))
                .collect(),
            methods: config
                .methods
                .iter()
                .map(|name| Symbol::intern(name))
                .collect(),
        }
    }
}
